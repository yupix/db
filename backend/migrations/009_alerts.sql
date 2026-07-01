-- Per-project metric alert rules. Evaluated by the metrics collector on every
-- sample; `triggered` reflects the latest evaluation so the UI can show which
-- rules are currently firing.
CREATE TABLE metric_alerts (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id        UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    metric            TEXT NOT NULL,              -- 'cpu_pct' | 'mem_pct'
    comparison        TEXT NOT NULL DEFAULT 'gt', -- 'gt' | 'lt'
    threshold         DOUBLE PRECISION NOT NULL,  -- percent (0-100)
    enabled           BOOLEAN NOT NULL DEFAULT true,
    triggered         BOOLEAN NOT NULL DEFAULT false,
    last_triggered_at TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_metric_alerts_project ON metric_alerts(project_id);
