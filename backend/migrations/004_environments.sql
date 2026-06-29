-- 004_environments.sql
CREATE TABLE project_environments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    -- e.g., "development", "staging", "production"
    endpoint_type   TEXT NOT NULL DEFAULT 'direct'
        CHECK (endpoint_type IN ('direct', 'pooled')),
    connection_string TEXT NOT NULL,
    is_default      BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(project_id, name)
);

CREATE INDEX idx_project_env_project_id ON project_environments(project_id);

-- Only one default environment per project
CREATE UNIQUE INDEX idx_project_env_one_default
    ON project_environments(project_id)
    WHERE is_default = true;
