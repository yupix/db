-- Backup snapshots (pg_dump custom-format archives, stored on the host disk
-- outside any project container so they survive container/project deletion
-- until explicitly pruned).
CREATE TABLE backups (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    branch_id   UUID REFERENCES branches(id) ON DELETE SET NULL,
    file_path   TEXT NOT NULL,
    size_bytes  BIGINT,
    status      TEXT NOT NULL DEFAULT 'creating',
    -- creating | completed | failed | restoring
    kind        TEXT NOT NULL DEFAULT 'manual',
    -- manual | scheduled
    error       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_backups_project ON backups(project_id, created_at DESC);

-- Retention policy per project (7.5-style per-project config). NULL fields
-- mean "don't keep this bucket" so a project can opt out of any tier.
CREATE TABLE backup_policies (
    project_id      UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    enabled         BOOLEAN NOT NULL DEFAULT false,
    schedule_hour   INT NOT NULL DEFAULT 3, -- hour of day (UTC) daily snapshot runs at
    daily_keep      INT NOT NULL DEFAULT 7,
    weekly_keep     INT NOT NULL DEFAULT 4,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
