-- 005_branches.sql
CREATE TABLE branches (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_branch_id UUID REFERENCES branches(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    container_id    TEXT,
    container_name  TEXT UNIQUE NOT NULL,
    port            INT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'creating',
    -- creating | running | stopped | error | deleted
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_branches_project_id ON branches(project_id);
CREATE INDEX idx_branches_parent_id ON branches(parent_branch_id);
CREATE INDEX idx_branches_status ON branches(status);
