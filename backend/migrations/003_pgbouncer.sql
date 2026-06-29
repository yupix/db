-- 003_pgbouncer.sql
-- PgBouncer関連フィールド追加
ALTER TABLE projects
    ADD COLUMN pgbouncer_container_id TEXT,
    ADD COLUMN pgbouncer_port INT,
    ADD COLUMN docker_network_id TEXT,
    ADD COLUMN pool_mode TEXT NOT NULL DEFAULT 'transaction',
    ADD COLUMN max_client_conn INT NOT NULL DEFAULT 100,
    ADD COLUMN default_pool_size INT NOT NULL DEFAULT 20;
