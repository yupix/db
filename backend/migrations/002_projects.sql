-- 002_projects.sql
CREATE TABLE projects (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    slug            TEXT UNIQUE NOT NULL,
    container_id    TEXT,
    container_name  TEXT UNIQUE NOT NULL,
    db_name         TEXT NOT NULL,
    db_user         TEXT NOT NULL,
    db_password     TEXT NOT NULL,
    port            INT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'creating',
    -- creating | running | stopped | error | deleted
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_projects_user_id ON projects(user_id);
CREATE INDEX idx_projects_slug ON projects(slug);
CREATE INDEX idx_projects_status ON projects(status);
