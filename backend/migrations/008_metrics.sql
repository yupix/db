-- Raw container resource samples, collected periodically by the metrics
-- collector. Kept short-term (pruned ~24h); older data lives in the rollup.
CREATE TABLE project_metrics (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id        UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ts                TIMESTAMPTZ NOT NULL DEFAULT now(),
    cpu_pct           DOUBLE PRECISION NOT NULL,
    mem_used_bytes    BIGINT NOT NULL,
    mem_limit_bytes   BIGINT NOT NULL,
    net_rx_bytes      BIGINT NOT NULL,
    net_tx_bytes      BIGINT NOT NULL,
    block_read_bytes  BIGINT NOT NULL,
    block_write_bytes BIGINT NOT NULL
);

CREATE INDEX idx_project_metrics_project_ts ON project_metrics(project_id, ts DESC);

-- Hourly rollup (averages + peaks). Raw samples are aggregated into this table
-- by the rollup task, allowing long-range history without unbounded raw rows.
CREATE TABLE project_metrics_rollup (
    project_id        UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    bucket            TIMESTAMPTZ NOT NULL, -- truncated to the hour
    samples           INT NOT NULL,
    cpu_pct_avg       DOUBLE PRECISION NOT NULL,
    cpu_pct_max       DOUBLE PRECISION NOT NULL,
    mem_used_avg      BIGINT NOT NULL,
    mem_used_max      BIGINT NOT NULL,
    mem_limit_bytes   BIGINT NOT NULL,
    net_rx_bytes      BIGINT NOT NULL,
    net_tx_bytes      BIGINT NOT NULL,
    block_read_bytes  BIGINT NOT NULL,
    block_write_bytes BIGINT NOT NULL,
    PRIMARY KEY (project_id, bucket)
);

CREATE INDEX idx_project_metrics_rollup_bucket ON project_metrics_rollup(project_id, bucket DESC);
