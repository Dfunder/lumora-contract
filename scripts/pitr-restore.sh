#!/usr/bin/env bash
# =============================================================================
# pitr-restore.sh — Point-in-Time Recovery for the lumora-pg CloudNativePG cluster
#
# What it does
# ------------
#   1. Validates inputs and checks cluster health.
#   2. Scales down consumers (PgBouncer) to avoid split-brain writes.
#   3. Patches the CloudNativePG Cluster CR with a recovery target timestamp.
#   4. Monitors the bootstrap phase until the recovered cluster is ready.
#   5. Scales PgBouncer back up once the cluster accepts connections.
#
# Usage
# -----
#   ./scripts/pitr-restore.sh --target-time "2026-07-24 18:30:00+00"
#
# Options
#   --target-time   UTC timestamp to recover to (ISO 8601 / PG format)
#                   Example: "2026-07-24 18:30:00+00"
#   --namespace     Kubernetes namespace (default: lumora-db)
#   --cluster       CloudNativePG cluster name (default: lumora-pg)
#   --dry-run       Print the patch YAML without applying it
#   --help          Show this help message
#
# Pre-requisites
# --------------
#   kubectl ≥ 1.28, jq ≥ 1.6, access to the lumora-db namespace.
#
# WARNING
# -------
#   PITR replaces the current cluster data with a restored copy.
#   ALL DATA WRITTEN AFTER THE TARGET TIME WILL BE LOST.
#   Ensure you have an off-cluster backup or snapshot before running.
# =============================================================================

set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────
NAMESPACE="lumora-db"
CLUSTER_NAME="lumora-pg"
PGBOUNCER_DEPLOY="pgbouncer"
DRY_RUN=false
TARGET_TIME=""

# ── Colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; NC='\033[0m'
info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
die()   { error "$*"; exit 1; }

# ── Parse args ────────────────────────────────────────────────────────────────
usage() {
  sed -n '/^# Usage/,/^# =\{10\}/p' "$0" | head -n -1
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target-time) TARGET_TIME="$2"; shift 2 ;;
    --namespace)   NAMESPACE="$2";   shift 2 ;;
    --cluster)     CLUSTER_NAME="$2"; shift 2 ;;
    --dry-run)     DRY_RUN=true;      shift   ;;
    --help|-h)     usage ;;
    *) die "Unknown argument: $1" ;;
  esac
done

# ── Validate inputs ───────────────────────────────────────────────────────────
[[ -z "$TARGET_TIME" ]] && die "--target-time is required. Example: --target-time '2026-07-24 18:30:00+00'"

# Validate that kubectl is available
command -v kubectl &>/dev/null || die "kubectl not found in PATH"
command -v jq     &>/dev/null || die "jq not found in PATH"

# Validate timestamp format (accept ISO 8601 with offset)
if ! date -d "$TARGET_TIME" &>/dev/null 2>&1; then
  die "Invalid --target-time value: '$TARGET_TIME'. Use format: 'YYYY-MM-DD HH:MM:SS+00'"
fi

# ── Confirm (interactive) ─────────────────────────────────────────────────────
echo ""
warn "┌────────────────────────────────────────────────────────────────────┐"
warn "│  POINT-IN-TIME RECOVERY — DESTRUCTIVE OPERATION                   │"
warn "│  Cluster : $CLUSTER_NAME (namespace: $NAMESPACE)"
warn "│  Target  : $TARGET_TIME"
warn "│  All data written after the target time will be PERMANENTLY LOST. │"
warn "└────────────────────────────────────────────────────────────────────┘"
echo ""

if [[ "$DRY_RUN" == "false" ]]; then
  read -r -p "Type 'yes-restore' to continue: " CONFIRM
  [[ "$CONFIRM" == "yes-restore" ]] || die "Aborted by user."
fi

# ── Step 1: Check cluster exists ─────────────────────────────────────────────
info "Checking cluster '$CLUSTER_NAME' in namespace '$NAMESPACE'..."
if ! kubectl get cluster.postgresql.cnpg.io "$CLUSTER_NAME" -n "$NAMESPACE" &>/dev/null; then
  die "Cluster '$CLUSTER_NAME' not found in namespace '$NAMESPACE'."
fi

CURRENT_STATUS=$(kubectl get cluster.postgresql.cnpg.io "$CLUSTER_NAME" \
  -n "$NAMESPACE" -o jsonpath='{.status.phase}')
info "Current cluster phase: $CURRENT_STATUS"

# ── Step 2: Scale down PgBouncer (prevent new writes) ────────────────────────
if [[ "$DRY_RUN" == "false" ]]; then
  info "Scaling down PgBouncer to 0 replicas..."
  kubectl scale deployment "$PGBOUNCER_DEPLOY" -n "$NAMESPACE" --replicas=0
  kubectl rollout status deployment/"$PGBOUNCER_DEPLOY" -n "$NAMESPACE" --timeout=60s || \
    warn "PgBouncer did not scale down cleanly; continuing anyway."
fi

# ── Step 3: Build the recovery patch ─────────────────────────────────────────
RECOVERY_PATCH=$(cat <<EOF
spec:
  bootstrap:
    recovery:
      source: lumora-pg
      recoveryTarget:
        targetTime: "${TARGET_TIME}"
  externalClusters:
    - name: lumora-pg
      barmanObjectStore:
        destinationPath: "s3://lumora-wal/backups"
        endpointURL: "http://minio.lumora-db.svc.cluster.local:9000"
        s3Credentials:
          accessKeyId:
            name: minio-s3-secret
            key: ACCESS_KEY_ID
          secretAccessKey:
            name: minio-s3-secret
            key: SECRET_ACCESS_KEY
        wal:
          maxParallel: 8
EOF
)

if [[ "$DRY_RUN" == "true" ]]; then
  echo ""
  info "DRY RUN — patch that would be applied:"
  echo "---"
  echo "$RECOVERY_PATCH"
  echo "---"
  info "Dry run complete. Exiting."
  exit 0
fi

# ── Step 4: Apply the recovery patch ─────────────────────────────────────────
info "Applying recovery bootstrap patch to cluster '$CLUSTER_NAME'..."
echo "$RECOVERY_PATCH" | kubectl patch cluster.postgresql.cnpg.io "$CLUSTER_NAME" \
  -n "$NAMESPACE" --type merge --patch-file /dev/stdin

# CloudNativePG requires deleting all pods to restart the bootstrap process
info "Deleting all cluster pods to trigger recovery bootstrap..."
kubectl delete pod -n "$NAMESPACE" \
  -l "cnpg.io/cluster=${CLUSTER_NAME}" --wait=false

# ── Step 5: Monitor recovery ──────────────────────────────────────────────────
info "Waiting for cluster to enter 'Setting up primary' phase..."
TIMEOUT=300  # 5 minutes
ELAPSED=0
POLL=10

while true; do
  PHASE=$(kubectl get cluster.postgresql.cnpg.io "$CLUSTER_NAME" \
    -n "$NAMESPACE" -o jsonpath='{.status.phase}' 2>/dev/null || echo "unknown")
  READY=$(kubectl get cluster.postgresql.cnpg.io "$CLUSTER_NAME" \
    -n "$NAMESPACE" -o jsonpath='{.status.readyInstances}' 2>/dev/null || echo "0")

  info "  Phase: $PHASE | Ready instances: $READY / 3"

  if [[ "$PHASE" == "Cluster in healthy state" ]] && [[ "$READY" -ge 1 ]]; then
    break
  fi

  if [[ "$ELAPSED" -ge "$TIMEOUT" ]]; then
    die "Cluster did not recover within ${TIMEOUT}s. Check 'kubectl describe cluster $CLUSTER_NAME -n $NAMESPACE'."
  fi

  sleep "$POLL"
  ELAPSED=$((ELAPSED + POLL))
done

# ── Step 6: Scale PgBouncer back up ──────────────────────────────────────────
info "Recovery complete. Scaling PgBouncer back to 2 replicas..."
kubectl scale deployment "$PGBOUNCER_DEPLOY" -n "$NAMESPACE" --replicas=2
kubectl rollout status deployment/"$PGBOUNCER_DEPLOY" -n "$NAMESPACE" --timeout=120s

echo ""
info "✅  PITR restore finished successfully."
info "    Recovered to: $TARGET_TIME"
info "    Cluster status: $(kubectl get cluster.postgresql.cnpg.io "$CLUSTER_NAME" \
  -n "$NAMESPACE" -o jsonpath='{.status.phase}')"
echo ""
warn "Next steps:"
warn "  1. Verify application data at the recovered timestamp."
warn "  2. Remove the 'externalClusters' / recovery bootstrap section from the CR"
warn "     to allow the cluster to take new base backups going forward."
warn "  3. Run: kubectl patch cluster.postgresql.cnpg.io $CLUSTER_NAME -n $NAMESPACE"
warn "          --type merge -p '{\"spec\":{\"bootstrap\":{\"recovery\":null},\"externalClusters\":null}}'"
