# PostgreSQL HA Architecture & Runbook

## Overview

This document describes the self-hosted, high-availability PostgreSQL infrastructure for the Lumora platform.  
The stack is vendor-agnostic and runs entirely on Kubernetes using:

| Component | Role |
|-----------|------|
| **CloudNativePG** | Kubernetes operator managing the PostgreSQL cluster lifecycle |
| **PostgreSQL 16** | Primary database — 1 primary + 2 streaming replicas |
| **PgBouncer** | Connection pooler in transaction mode (handles thousands of app connections) |
| **MinIO** | S3-compatible object store for WAL archiving and base backups |

---

## Architecture Diagram

```
┌───────────────────────────────────────────────────────────────────────┐
│  Kubernetes Cluster                                                   │
│                                                                       │
│  ┌─────────────────────────────────────┐   ┌──────────────────────┐  │
│  │ App Pods (any namespace)            │   │ CloudNativePG Operator│  │
│  │                                     │   │  (cnpg-system ns)    │  │
│  │  DSN: pgbouncer.lumora-db:5432/lumora│  └──────────┬───────────┘  │
│  └──────────────┬──────────────────────┘             │              │
│                 │                                     │              │
│  ┌──────────────▼──────────────────────────────────┐ │              │
│  │          PgBouncer (2 pods, anti-affinity)       │ │              │
│  │          transaction-pooling mode                │ │              │
│  │          max_client_conn=2000  pool_size=25       │ │              │
│  └──────┬────────────────────────────┬─────────────┘ │              │
│         │ rw                         │ ro             │              │
│  ┌──────▼──────┐            ┌────────▼──────┐        │              │
│  │ lumora-pg-rw│            │lumora-pg-ro   │        │              │
│  │ (Service)   │            │(Service)      │        │              │
│  └──────┬──────┘            └────────┬──────┘        │              │
│         │                           │                │              │
│  ┌──────▼──────────────────────────────────────────┐ │              │
│  │              lumora-pg Cluster (3 pods)         │◄┘              │
│  │                                                 │                │
│  │  ┌─────────┐  ┌───────────┐  ┌───────────┐     │                │
│  │  │ Primary │─►│ Replica-1 │  │ Replica-2 │     │                │
│  │  │  (rw)   │  │ (standby) │  │ (standby) │     │                │
│  │  └────┬────┘  └───────────┘  └───────────┘     │                │
│  └───────┼─────────────────────────────────────────┘                │
│          │ WAL stream                                                │
│          ▼                                                           │
│  ┌───────────────┐                                                   │
│  │ MinIO         │  s3://lumora-wal/backups                          │
│  │ (lumora-db ns)│  WAL segments + daily base backups                │
│  └───────────────┘                                                   │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Components & Configuration

### CloudNativePG Cluster (`cnpg-cluster.yaml`)

- **3 instances**: 1 primary + 2 streaming standbys.
- **Synchronous commit**: `remote_write` — data is durable on at least one standby before the primary acknowledges.
- **Automatic failover**: CloudNativePG monitors the primary via its instance manager. A standby is promoted within **~10 seconds** of the primary becoming unresponsive (satisfies the ≤10 s acceptance criterion).
- **WAL archiving**: Continuous WAL segments are shipped to MinIO via Barman Cloud (`barman-cloud-wal-archive`).
- **Daily base backup**: A `ScheduledBackup` CR triggers a full base backup every day at 02:00 UTC.
- **Retention**: 14 days of WAL + base backups.

### PgBouncer (`pgbouncer.yaml`)

- **2 replicas** with pod anti-affinity (one per node).
- **Transaction pooling mode**: each server connection serves many client connections, dramatically reducing PostgreSQL's connection overhead.
- **Pools**:
  - `lumora` → `lumora-pg-rw` (read-write primary)
  - `lumora_ro` → `lumora-pg-ro` (read-only replicas, for reporting/analytics)
- **Connection limits**: `max_client_conn=2000`, `default_pool_size=25`.
- **PodDisruptionBudget**: ensures at least 1 PgBouncer pod survives node drains.
- **Prometheus exporter** sidecar on `:9127`.

### MinIO (`minio.yaml`)

- Single-node MinIO suitable for dev/staging; swap for a distributed MinIO cluster or AWS S3 in production.
- A bootstrap Job creates the `lumora-wal` bucket on first deployment.
- Credentials stored in the `minio-s3-secret` Kubernetes Secret.

---

## Deployment

### Pre-requisites

```bash
# Kubernetes ≥ 1.28, kubectl, helm ≥ 3.12
kubectl version --short
helm version --short

# Install the CloudNativePG operator
helm repo add cnpg https://cloudnative-pg.github.io/charts
helm repo update
helm upgrade --install cnpg cnpg/cloudnative-pg \
  --namespace cnpg-system --create-namespace \
  -f deployments/postgres/helm/cnpg-operator-values.yaml
```

### Step-by-step

```bash
# 1. Create namespace
kubectl create namespace lumora-db --dry-run=client -o yaml | kubectl apply -f -

# 2. Update secrets with real passwords (do NOT commit plaintext passwords)
#    Consider using Sealed Secrets, External Secrets Operator, or Vault.
kubectl apply -f deployments/postgres/minio.yaml          # MinIO + secrets
kubectl apply -f deployments/postgres/cnpg-cluster.yaml   # CNPG cluster + secrets
kubectl apply -f deployments/postgres/pgbouncer.yaml      # PgBouncer + admin secret

# 3. Wait for the cluster to be ready
kubectl wait cluster/lumora-pg -n lumora-db \
  --for=condition=Ready --timeout=300s

# 4. Verify
kubectl get cluster lumora-pg -n lumora-db
kubectl get pods -n lumora-db
```

### Verify replication

```bash
# Connect via PgBouncer
kubectl run pg-client --rm -it --image=postgres:16 --restart=Never -- \
  psql -h pgbouncer.lumora-db -U lumora_app -d lumora

# Inside psql — check replication lag
SELECT application_name, state, sent_lsn, write_lsn,
       flush_lsn, replay_lsn, sync_state
FROM pg_stat_replication;
```

---

## Operational Runbook

### Failover test

```bash
# Identify the primary pod
kubectl get pods -n lumora-db -l cnpg.io/cluster=lumora-pg \
  -o jsonpath='{range .items[?(@.metadata.labels.cnpg\.io/instanceRole=="primary")]}{.metadata.name}{"\n"}{end}'

# Delete the primary pod — CloudNativePG promotes a replica within ~10 s
kubectl delete pod <primary-pod-name> -n lumora-db

# Watch the promotion event
kubectl get events -n lumora-db --watch | grep -i failover

# Confirm new primary
kubectl get cluster lumora-pg -n lumora-db -o jsonpath='{.status.currentPrimary}'
```

### Scale read replicas

```bash
# Edit the cluster spec — change instances: 3 → 5
kubectl patch cluster lumora-pg -n lumora-db \
  --type merge -p '{"spec":{"instances":5}}'
```

### Manual base backup

```bash
cat <<EOF | kubectl apply -f -
apiVersion: postgresql.cnpg.io/v1
kind: Backup
metadata:
  name: lumora-pg-manual-$(date +%Y%m%d)
  namespace: lumora-db
spec:
  method: barmanObjectStore
  cluster:
    name: lumora-pg
EOF
```

### Point-in-Time Recovery (PITR)

```bash
# Restore to a specific timestamp
./scripts/pitr-restore.sh --target-time "2026-07-24 18:30:00+00"

# Dry run (shows patch without applying)
./scripts/pitr-restore.sh --target-time "2026-07-24 18:30:00+00" --dry-run
```

See `scripts/pitr-restore.sh` for full options and a description of each step.

### PgBouncer pool health check

```bash
# Port-forward the PgBouncer admin interface
kubectl port-forward svc/pgbouncer 5432:5432 -n lumora-db &

# Connect to the admin virtual database
PGPASSWORD=<ADMIN_PASSWORD> psql -h localhost -p 5432 \
  -U pgbouncer_admin -d pgbouncer

# Useful commands inside psql:
# SHOW POOLS;    — connection counts per pool
# SHOW STATS;    — throughput / latency metrics
# SHOW CLIENTS;  — active client connections
# SHOW SERVERS;  — active server connections
# RELOAD;        — reload pgbouncer.ini without restart
```

### PgBouncer load test

```bash
# Run from inside the cluster (or with kubectl port-forward)
export PGPASSWORD=<APP_PASSWORD>
./scripts/pgbouncer-load-test.sh \
  --host localhost --port 5432 \
  --clients 200 --duration 60

# High-load stress test
./scripts/pgbouncer-load-test.sh \
  --host localhost --port 5432 \
  --clients 1000 --duration 120 --threads 16 --skip-init
```

---

## Acceptance Criteria Verification

| Criterion | How to verify |
|-----------|--------------|
| PostgreSQL cluster recovers from primary pod termination within 10 s | `kubectl delete pod <primary>` then watch `kubectl get cluster lumora-pg -n lumora-db -w` — `currentPrimary` changes within ≤10 s |
| PgBouncer maintains stable connection pools under synthetic load | Run `scripts/pgbouncer-load-test.sh --clients 200 --duration 60`; check zero waiting clients in `SHOW POOLS` |
| PITR restore script recovers state from WAL archives | Run `scripts/pitr-restore.sh --target-time "<timestamp>"` on a test cluster; verify data state matches expected point-in-time |

---

## Security Notes

- **Never commit plaintext passwords** to Git. Use [Sealed Secrets](https://github.com/bitnami-labs/sealed-secrets), [External Secrets Operator](https://external-secrets.io/), or HashiCorp Vault.
- Enable TLS between PgBouncer and PostgreSQL in production by uncommenting `server_tls_sslmode = require` in `pgbouncer.ini`.
- Restrict MinIO access to the `lumora-db` namespace with a `NetworkPolicy`.
- Rotate credentials after initial setup using `kubectl create secret --dry-run=client -o yaml | kubectl apply -f -`.

---

## References

- [CloudNativePG documentation](https://cloudnative-pg.io/documentation/)
- [PgBouncer configuration reference](https://www.pgbouncer.org/config.html)
- [MinIO Kubernetes deployment](https://min.io/docs/minio/kubernetes/upstream/)
- [Barman Cloud WAL archiving](https://cloudnative-pg.io/documentation/current/backup_recovery/)
