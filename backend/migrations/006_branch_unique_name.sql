-- 006_branch_unique_name.sql
CREATE UNIQUE INDEX idx_branches_project_name
    ON branches(project_id, name)
    WHERE status != 'deleted';
