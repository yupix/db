use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::Project;
use crate::error::AppError;

/// Access level required against a project.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access {
    /// Read-only: project owner, the org owner, or ANY member of a team the
    /// project is assigned to (`project_teams`).
    Read,
    /// Mutating: project owner, the org owner, or a team `owner`/`admin` of a
    /// team the project is assigned to.
    Manage,
}

/// Fetch a project the user is allowed to access at the given level.
///
/// Honors both direct ownership (`projects.user_id`) and team assignment via
/// `project_teams`. Returns `NotFound` when the project does not exist OR the
/// user lacks access at `access` — we deliberately do not distinguish the two
/// so existence of other users' projects is not leaked.
pub async fn fetch_project_for(
    db: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    access: Access,
) -> Result<Project, AppError> {
    // Admins/owners may manage; for read, any team member qualifies.
    let member_role_filter = match access {
        Access::Read => "",
        Access::Manage => "AND tm.role IN ('owner', 'admin')",
    };

    let sql = format!(
        "SELECT p.* FROM projects p
         WHERE p.id = $1 AND p.status != 'deleted'
           AND (
             p.user_id = $2
             OR EXISTS (
               SELECT 1 FROM project_teams pt
               JOIN team_members tm ON tm.team_id = pt.team_id
               WHERE pt.project_id = p.id AND tm.user_id = $2 {member_role_filter}
             )
             OR EXISTS (
               SELECT 1 FROM project_teams pt
               JOIN teams t ON t.id = pt.team_id
               JOIN organizations o ON o.id = t.org_id
               WHERE pt.project_id = p.id AND o.owner_id = $2
             )
           )"
    );

    sqlx::query_as::<_, Project>(&sql)
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}
