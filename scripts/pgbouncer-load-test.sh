#!/usr/bin/env bash
# =============================================================================
# pgbouncer-load-test.sh — Synthetic load test for PgBouncer stability
#
# What it does
# ------------
#   1. Runs pgbench to simulate concurrent client connections through PgBouncer.
#   2. Captures pgbouncer SHOW STATS between phases to validate pool stability.
#   3. Reports TPS, latency, pool hit-rate and flags any error thresholds.
#
# Usage
# -----
#   ./scripts/pgbouncer-load-test.sh [OPTIONS]
#
# Options
#   --host         PgBouncer host (default: pgbouncer.lumora-db.svc.cluster.local)
#   --port         PgBouncer port (default: 5432)
#   --db           Database name (default: lumora)
#   --user         Database user (default: lumora_app)
#   --duration     Test duration in seconds (default: 60)
#   --clients      Number of concurrent clients (default: 200)
#   --threads      pgbench worker threads (default: 8)
#   --scale        pgbench scale factor for init (default: 10)
#   --skip-init    Skip pgbench schema initialisation
#   --help         Show this help
#
# Pre-requisites
# --------------
#   pgbench (ships with postgresql-client), psql, kubectl (for port-forward),
#   PGPASSWORD env var set, or a ~/.pgpass entry for the user.
# =============================================================================

set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────
HOST="${PGBOUNCER_HOST:-pgbouncer.lumora-db.svc.cluster.local}"
PORT="${PGBOUNCER_PORT:-5432}"
DB="${PGBOUNCER_DB:-lumora}"
USER="${PGBOUNCER_USER:-lumora_app}"
DURATION=60
CLIENTS=200
THREADS=8
SCALE=10
SKIP_INIT=false

# ── Thresholds ────────────────────────────────────────────────────────────────
MAX_ERROR_RATE_PCT=1    # fail if pgbench error rate exceeds 1 %
MIN_TPS=100             # warn if TPS falls below this value

# ── Colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
header(){ echo -e "\n${CYAN}══════════════════════════════════════════════${NC}"; echo -e "${CYAN}  $*${NC}"; echo -e "${CYAN}══════════════════════════════════════════════${NC}"; }
die()   { error "$*"; exit 1; }

# ── Parse args ────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)      HOST="$2";      shift 2 ;;
    --port)      PORT="$2";      shift 2 ;;
    --db)        DB="$2";        shift 2 ;;
    --user)      USER="$2";      shift 2 ;;
    --duration)  DURATION="$2";  shift 2 ;;
    --clients)   CLIENTS="$2";   shift 2 ;;
    --threads)   THREADS="$2";   shift 2 ;;
    --scale)     SCALE="$2";     shift 2 ;;
    --skip-init) SKIP_INIT=true; shift   ;;
    --help|-h)   sed -n '/^# Usage/,/^# =\{10\}/p' "$0" | head -n -1; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
done

# ── Pre-flight checks ─────────────────────────────────────────────────────────
command -v pgbench &>/dev/null || die "pgbench not found. Install postgresql-client."
command -v psql    &>/dev/null || die "psql not found. Install postgresql-client."
command -v jq      &>/dev/null || die "jq not found."
command -v bc      &>/dev/null || die "bc not found."

[[ -z "${PGPASSWORD:-}" ]] && warn "PGPASSWORD is not set. psql/pgbench will prompt for password or use ~/.pgpass."

CONN_ARGS="-h $HOST -p $PORT -U $USER"

header "Lumora PgBouncer Load Test"
info "Target     : $USER@$HOST:$PORT/$DB"
info "Clients    : $CLIENTS"
info "Threads    : $THREADS"
info "Duration   : ${DURATION}s"
info "Scale      : $SCALE"
info "Skip init  : $SKIP_INIT"
echo ""

# ── Helper: run SQL against pgbouncer admin db ────────────────────────────────
pgb_sql() {
  # pgbouncer admin interface listens on the pgbouncer virtual database
  psql $CONN_ARGS -d pgbouncer -tAc "$1" 2>/dev/null || echo ""
}

# ── Step 1: Connectivity check ────────────────────────────────────────────────
header "Step 1: Connectivity"
info "Testing connection to PgBouncer..."
CONN_TEST=$(psql $CONN_ARGS -d "$DB" -tAc "SELECT 1" 2>&1) || true
if [[ "$CONN_TEST" == "1" ]]; then
  info "✅  Connection OK"
else
  die "Cannot connect to PgBouncer: $CONN_TEST"
fi

PG_VERSION=$(psql $CONN_ARGS -d "$DB" -tAc "SELECT version()" | head -1)
info "Server: $PG_VERSION"

# ── Step 2: Initialise pgbench schema ─────────────────────────────────────────
if [[ "$SKIP_INIT" == "false" ]]; then
  header "Step 2: Initialise pgbench schema (scale=$SCALE)"
  warn "This will create pgbench_* tables in the '$DB' database."
  pgbench $CONN_ARGS -d "$DB" --initialize --scale="$SCALE" --quiet
  info "✅  pgbench schema ready."
else
  info "Skipping schema init (--skip-init)."
fi

# ── Step 3: Baseline pool stats (before load) ─────────────────────────────────
header "Step 3: Baseline PgBouncer pool stats"
STATS_BEFORE=$(pgb_sql "SHOW POOLS;") || true
if [[ -n "$STATS_BEFORE" ]]; then
  echo "$STATS_BEFORE"
else
  warn "Could not retrieve SHOW POOLS (stats user may not be configured yet)."
fi

# ── Step 4: Warm-up run (10 s, 10 clients) ───────────────────────────────────
header "Step 4: Warm-up (10s, 10 clients)"
pgbench $CONN_ARGS -d "$DB" \
  --time=10 --client=10 --jobs=2 \
  --progress=5 --no-vacuum --quiet 2>&1 | tail -5 || true
info "Warm-up complete."

# ── Step 5: Main load test ────────────────────────────────────────────────────
header "Step 5: Main load test (${DURATION}s, ${CLIENTS} clients)"
info "Starting pgbench..."

RESULTS_FILE=$(mktemp /tmp/pgbench-results-XXXXXX.txt)
# shellcheck disable=SC2064
trap "rm -f $RESULTS_FILE" EXIT

pgbench $CONN_ARGS -d "$DB" \
  --time="$DURATION" \
  --client="$CLIENTS" \
  --jobs="$THREADS" \
  --progress=10 \
  --no-vacuum \
  --log --log-prefix=/tmp/pgbench-log \
  2>&1 | tee "$RESULTS_FILE"

# ── Step 6: Parse results ─────────────────────────────────────────────────────
header "Step 6: Results"

TPS=$(grep -E "^tps = " "$RESULTS_FILE" | grep "excluding" | awk '{print $3}' | tr -d ',' || echo "0")
LATENCY=$(grep -E "^latency average" "$RESULTS_FILE" | awk '{print $3}' || echo "0")
LATENCY_STDDEV=$(grep -E "^latency stddev" "$RESULTS_FILE" | awk '{print $3}' || echo "0")
ERRORS=$(grep -E "^number of failed transactions" "$RESULTS_FILE" | awk '{print $NF}' || echo "0")
TRANSACTIONS=$(grep -E "^number of transactions" "$RESULTS_FILE" | awk '{print $NF}' || echo "0")

info "TPS (excl. connect): ${TPS}"
info "Latency avg        : ${LATENCY} ms"
info "Latency stddev     : ${LATENCY_STDDEV} ms"
info "Transactions       : ${TRANSACTIONS}"
info "Errors             : ${ERRORS}"

# Error rate check
if [[ "$TRANSACTIONS" -gt 0 && "$ERRORS" -gt 0 ]]; then
  ERROR_RATE=$(echo "scale=2; $ERRORS * 100 / $TRANSACTIONS" | bc)
  if (( $(echo "$ERROR_RATE > $MAX_ERROR_RATE_PCT" | bc -l) )); then
    error "❌  Error rate ${ERROR_RATE}% exceeds threshold ${MAX_ERROR_RATE_PCT}%"
    EXIT_CODE=1
  else
    info "✅  Error rate ${ERROR_RATE}% within threshold (≤${MAX_ERROR_RATE_PCT}%)"
    EXIT_CODE=0
  fi
else
  info "✅  No transaction errors."
  EXIT_CODE=0
fi

# TPS check
if (( $(echo "$TPS < $MIN_TPS" | bc -l) )); then
  warn "⚠️   TPS ${TPS} is below minimum expected ${MIN_TPS}. Check cluster sizing."
else
  info "✅  TPS ${TPS} meets minimum threshold (≥${MIN_TPS})"
fi

# ── Step 7: Pool stats after load ─────────────────────────────────────────────
header "Step 7: Post-test PgBouncer pool stats"
STATS_AFTER=$(pgb_sql "SHOW POOLS;") || true
if [[ -n "$STATS_AFTER" ]]; then
  echo "$STATS_AFTER"

  # Check for waiting clients (sign of pool exhaustion)
  WAITING=$(pgb_sql "SHOW POOLS;" | awk -F'|' 'NR>2 { gsub(/ /, "", $8); sum += $8 } END { print sum+0 }') || echo "0"
  if [[ "$WAITING" -gt 0 ]]; then
    warn "⚠️   $WAITING clients are still waiting in the pool queue. Consider increasing pool_size."
  else
    info "✅  No waiting clients in pool."
  fi
else
  warn "Could not retrieve post-test pool stats."
fi

# SHOW STATS summary
pgb_sql "SHOW STATS;" || true

# ── Summary ────────────────────────────────────────────────────────────────────
header "Summary"
info "Test duration      : ${DURATION}s"
info "Concurrent clients : ${CLIENTS}"
info "TPS                : ${TPS}"
info "Avg latency        : ${LATENCY} ms"
info "Errors             : ${ERRORS} / ${TRANSACTIONS}"
echo ""

if [[ "${EXIT_CODE:-0}" -ne 0 ]]; then
  error "Load test FAILED — see errors above."
else
  info "✅  Load test PASSED."
fi

exit "${EXIT_CODE:-0}"
