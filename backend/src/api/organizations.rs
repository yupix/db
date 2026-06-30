use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::auth::jwt::Claims;
use crate::db::models::{Invitation, Organization, Team, TeamMember};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_orgs).post(create_org))
        .route("/{id}", get(get_org).patch(update_org).delete(delete_org))
        .route("/{id}/teams", get(list_teams).post(create_team))
        .route(
            "/{id}/teams/{team_id}",
            get(get_team).patch(update_team).delete(delete_team),
        )
        .route(
            "/{id}/teams/{team_id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/{id}/teams/{team_id}/members/{user_id}",
            delete(remove_member).patch(update_member_role),
        )
        .route(
            "/{id}/teams/{team_id}/invitations",
            get(list_invitations).post(create_invitation),
        )
        .route(
            "/{id}/teams/{team_id}/invitations/{inv_id}",
            delete(cancel_invitation),
        )
        .route(
            "/{id}/teams/{team_id}/projects",
            get(list_team_projects).post(assign_project),
        )
        .route(
            "/{id}/teams/{team_id}/projects/{project_id}",
            delete(unassign_project),
        )
        // Accept invitation (no org/team id needed — just the token)
        .route("/invitations/{token}/accept", post(accept_invitation))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn valid_role(role: &str) -> bool {
    matches!(role, "owner" | "admin" | "developer" | "viewer")
}

async fn require_org_member(
    state: &AppState,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Organization, AppError> {
    let org = sqlx::query_as::<_, Organization>("SELECT * FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let is_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM teams t
         JOIN team_members tm ON tm.team_id = t.id
         WHERE t.org_id = $1 AND tm.user_id = $2",
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    if org.owner_id != user_id && is_member == 0 {
        return Err(AppError::NotFound);
    }

    Ok(org)
}

async fn require_org_owner(
    state: &AppState,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Organization, AppError> {
    let org = sqlx::query_as::<_, Organization>("SELECT * FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if org.owner_id != user_id {
        return Err(AppError::Forbidden);
    }

    Ok(org)
}

/// Returns the caller's role in the team, or Forbidden if not a member.
/// Org owner is treated as "owner" even without explicit membership.
async fn require_team_member(
    state: &AppState,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<String, AppError> {
    let team = sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = $1")
        .bind(team_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if user is org owner
    let org_owner = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM organizations WHERE id = $1 AND owner_id = $2",
    )
    .bind(team.org_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    if org_owner > 0 {
        return Ok("owner".to_string());
    }

    let member = sqlx::query_as::<_, TeamMember>(
        "SELECT * FROM team_members WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Forbidden)?;

    Ok(member.role)
}

fn can_manage_members(role: &str) -> bool {
    matches!(role, "owner" | "admin")
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OrgResponse {
    id: String,
    name: String,
    slug: String,
    owner_id: String,
    created_at: String,
}

impl OrgResponse {
    fn from(o: &Organization) -> Self {
        Self {
            id: o.id.to_string(),
            name: o.name.clone(),
            slug: o.slug.clone(),
            owner_id: o.owner_id.to_string(),
            created_at: o.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct TeamResponse {
    id: String,
    org_id: String,
    name: String,
    created_at: String,
}

impl TeamResponse {
    fn from(t: &Team) -> Self {
        Self {
            id: t.id.to_string(),
            org_id: t.org_id.to_string(),
            name: t.name.clone(),
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct MemberResponse {
    id: String,
    team_id: String,
    user_id: String,
    role: String,
    email: String,
    name: String,
    created_at: String,
}

#[derive(Serialize)]
struct InvitationResponse {
    id: String,
    team_id: String,
    email: String,
    role: String,
    token: String,
    status: String,
    expires_at: String,
    created_at: String,
}

impl InvitationResponse {
    fn from(i: &Invitation) -> Self {
        Self {
            id: i.id.to_string(),
            team_id: i.team_id.to_string(),
            email: i.email.clone(),
            role: i.role.clone(),
            token: i.token.clone(),
            status: i.status.clone(),
            expires_at: i.expires_at.to_rfc3339(),
            created_at: i.created_at.to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Organization handlers
// ---------------------------------------------------------------------------

async fn list_orgs(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Vec<OrgResponse>>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let orgs = sqlx::query_as::<_, Organization>(
        "SELECT DISTINCT o.* FROM organizations o
         LEFT JOIN teams t ON t.org_id = o.id
         LEFT JOIN team_members tm ON tm.team_id = t.id AND tm.user_id = $1
         WHERE o.owner_id = $1 OR tm.user_id = $1
         ORDER BY o.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(orgs.iter().map(OrgResponse::from).collect()))
}

#[derive(Deserialize, Validate)]
struct CreateOrgRequest {
    #[validate(length(min = 1, max = 100))]
    name: String,
}

async fn create_org(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Json(req): Json<CreateOrgRequest>,
) -> Result<Json<OrgResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let slug = slugify_org(&req.name);
    let slug = if slug.is_empty() {
        format!("org-{}", &Uuid::new_v4().to_string().replace('-', "")[..8])
    } else {
        slug
    };

    let base_slug = slug.clone();
    let mut attempt_slug = base_slug.clone();
    let mut counter = 0u32;
    loop {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM organizations WHERE slug = $1",
        )
        .bind(&attempt_slug)
        .fetch_one(&state.db)
        .await?;

        if count == 0 {
            break;
        }
        counter += 1;
        attempt_slug = format!("{}-{}", base_slug, counter);
    }

    let org = sqlx::query_as::<_, Organization>(
        "INSERT INTO organizations (name, slug, owner_id) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&req.name)
    .bind(&attempt_slug)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(OrgResponse::from(&org)))
}

async fn get_org(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<OrgResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let org = require_org_member(&state, id, user_id).await?;
    Ok(Json(OrgResponse::from(&org)))
}

#[derive(Deserialize)]
struct UpdateOrgRequest {
    name: Option<String>,
}

async fn update_org(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateOrgRequest>,
) -> Result<Json<OrgResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_owner(&state, id, user_id).await?;

    if let Some(name) = &req.name {
        sqlx::query("UPDATE organizations SET name = $1, updated_at = now() WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    let org = sqlx::query_as::<_, Organization>("SELECT * FROM organizations WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(OrgResponse::from(&org)))
}

async fn delete_org(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_owner(&state, id, user_id).await?;

    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ---------------------------------------------------------------------------
// Team handlers
// ---------------------------------------------------------------------------

async fn list_teams(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<TeamResponse>>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_member(&state, org_id, user_id).await?;

    let teams = sqlx::query_as::<_, Team>(
        "SELECT * FROM teams WHERE org_id = $1 ORDER BY name",
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(teams.iter().map(TeamResponse::from).collect()))
}

#[derive(Deserialize, Validate)]
struct CreateTeamRequest {
    #[validate(length(min = 1, max = 100))]
    name: String,
}

async fn create_team(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(org_id): Path<Uuid>,
    Json(req): Json<CreateTeamRequest>,
) -> Result<Json<TeamResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_owner(&state, org_id, user_id).await?;

    let team = sqlx::query_as::<_, Team>(
        "INSERT INTO teams (org_id, name) VALUES ($1, $2) RETURNING *",
    )
    .bind(org_id)
    .bind(&req.name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.constraint().is_some_and(|c| c.contains("teams_org_id_name_key")) {
                return AppError::Conflict("Team name already exists in this organization".into());
            }
        }
        AppError::Database(e)
    })?;

    Ok(Json(TeamResponse::from(&team)))
}

async fn get_team(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<TeamResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_member(&state, org_id, user_id).await?;

    let team = sqlx::query_as::<_, Team>(
        "SELECT * FROM teams WHERE id = $1 AND org_id = $2",
    )
    .bind(team_id)
    .bind(org_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(TeamResponse::from(&team)))
}

#[derive(Deserialize)]
struct UpdateTeamRequest {
    name: Option<String>,
}

async fn update_team(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateTeamRequest>,
) -> Result<Json<TeamResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role = require_team_member(&state, team_id, user_id).await?;
    if !can_manage_members(&role) {
        return Err(AppError::Forbidden);
    }

    // Verify team belongs to org
    let team = sqlx::query_as::<_, Team>(
        "SELECT * FROM teams WHERE id = $1 AND org_id = $2",
    )
    .bind(team_id)
    .bind(org_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    if let Some(name) = &req.name {
        sqlx::query("UPDATE teams SET name = $1, updated_at = now() WHERE id = $2")
            .bind(name)
            .bind(team.id)
            .execute(&state.db)
            .await?;
    }

    let updated = sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = $1")
        .bind(team_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(TeamResponse::from(&updated)))
}

async fn delete_team(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_owner(&state, org_id, user_id).await?;

    let result = sqlx::query("DELETE FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(org_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ---------------------------------------------------------------------------
// Member handlers
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct MemberRow {
    id: Uuid,
    team_id: Uuid,
    user_id: Uuid,
    role: String,
    email: String,
    name: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_members(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<MemberResponse>>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_member(&state, org_id, user_id).await?;

    // Verify team belongs to org
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .and_then(|c| if c > 0 { Ok(c) } else { Err(sqlx::Error::RowNotFound) })
        .map_err(|_| AppError::NotFound)?;

    let rows = sqlx::query_as::<_, MemberRow>(
        "SELECT tm.id, tm.team_id, tm.user_id, tm.role,
                u.email, u.name, tm.created_at
         FROM team_members tm
         JOIN users u ON u.id = tm.user_id
         WHERE tm.team_id = $1
         ORDER BY tm.role, u.name",
    )
    .bind(team_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.iter()
            .map(|r| MemberResponse {
                id: r.id.to_string(),
                team_id: r.team_id.to_string(),
                user_id: r.user_id.to_string(),
                role: r.role.clone(),
                email: r.email.clone(),
                name: r.name.clone(),
                created_at: r.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}

#[derive(Deserialize, Validate)]
struct AddMemberRequest {
    #[validate(email)]
    email: String,
    role: Option<String>,
}

async fn add_member(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<AddMemberRequest>,
) -> Result<Json<MemberResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let caller_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role_str = require_team_member(&state, team_id, caller_id).await?;
    if !can_manage_members(&role_str) {
        return Err(AppError::Forbidden);
    }

    // Verify team belongs to org
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .and_then(|c| if c > 0 { Ok(c) } else { Err(sqlx::Error::RowNotFound) })
        .map_err(|_| AppError::NotFound)?;

    let role = req.role.unwrap_or_else(|| "developer".to_string());
    if !valid_role(&role) {
        return Err(AppError::BadRequest(
            "role must be one of: owner, admin, developer, viewer".into(),
        ));
    }

    let target_user = sqlx::query_as::<_, crate::db::models::User>(
        "SELECT * FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound)?;

    let member = sqlx::query_as::<_, TeamMember>(
        "INSERT INTO team_members (team_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (team_id, user_id) DO UPDATE SET role = EXCLUDED.role, updated_at = now()
         RETURNING *",
    )
    .bind(team_id)
    .bind(target_user.id)
    .bind(&role)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(MemberResponse {
        id: member.id.to_string(),
        team_id: member.team_id.to_string(),
        user_id: member.user_id.to_string(),
        role: member.role.clone(),
        email: target_user.email.clone(),
        name: target_user.name.clone(),
        created_at: member.created_at.to_rfc3339(),
    }))
}

#[derive(Deserialize)]
struct UpdateRoleRequest {
    role: String,
}

async fn update_member_role(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((_org_id, team_id, target_user_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let caller_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role_str = require_team_member(&state, team_id, caller_id).await?;
    if !can_manage_members(&role_str) {
        return Err(AppError::Forbidden);
    }

    if !valid_role(&req.role) {
        return Err(AppError::BadRequest(
            "role must be one of: owner, admin, developer, viewer".into(),
        ));
    }

    let result = sqlx::query(
        "UPDATE team_members SET role = $1, updated_at = now() WHERE team_id = $2 AND user_id = $3",
    )
    .bind(&req.role)
    .bind(team_id)
    .bind(target_user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "updated": true })))
}

async fn remove_member(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((_org_id, team_id, target_user_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let caller_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role_str = require_team_member(&state, team_id, caller_id).await?;
    if !can_manage_members(&role_str) {
        return Err(AppError::Forbidden);
    }

    let result =
        sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
            .bind(team_id)
            .bind(target_user_id)
            .execute(&state.db)
            .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ---------------------------------------------------------------------------
// Invitation handlers
// ---------------------------------------------------------------------------

async fn list_invitations(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<InvitationResponse>>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role = require_team_member(&state, team_id, user_id).await?;
    if !can_manage_members(&role) {
        return Err(AppError::Forbidden);
    }

    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .and_then(|c| if c > 0 { Ok(c) } else { Err(sqlx::Error::RowNotFound) })
        .map_err(|_| AppError::NotFound)?;

    let invitations = sqlx::query_as::<_, Invitation>(
        "SELECT * FROM invitations WHERE team_id = $1 AND status = 'pending' ORDER BY created_at DESC",
    )
    .bind(team_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(invitations.iter().map(InvitationResponse::from).collect()))
}

#[derive(Deserialize, Validate)]
struct CreateInvitationRequest {
    #[validate(email)]
    email: String,
    role: Option<String>,
}

async fn create_invitation(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<Json<InvitationResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role = require_team_member(&state, team_id, user_id).await?;
    if !can_manage_members(&role) {
        return Err(AppError::Forbidden);
    }

    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .and_then(|c| if c > 0 { Ok(c) } else { Err(sqlx::Error::RowNotFound) })
        .map_err(|_| AppError::NotFound)?;

    let inv_role = req.role.unwrap_or_else(|| "developer".to_string());
    if !valid_role(&inv_role) {
        return Err(AppError::BadRequest(
            "role must be one of: owner, admin, developer, viewer".into(),
        ));
    }

    // Expire any existing pending invitation for this email+team
    sqlx::query(
        "UPDATE invitations SET status = 'expired', updated_at = now()
         WHERE team_id = $1 AND email = $2 AND status = 'pending'",
    )
    .bind(team_id)
    .bind(&req.email)
    .execute(&state.db)
    .await?;

    let token = generate_token();

    let invitation = sqlx::query_as::<_, Invitation>(
        "INSERT INTO invitations (team_id, email, role, token, invited_by)
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(team_id)
    .bind(&req.email)
    .bind(&inv_role)
    .bind(&token)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(InvitationResponse::from(&invitation)))
}

async fn cancel_invitation(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((_org_id, team_id, inv_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role = require_team_member(&state, team_id, user_id).await?;
    if !can_manage_members(&role) {
        return Err(AppError::Forbidden);
    }

    let result = sqlx::query(
        "DELETE FROM invitations WHERE id = $1 AND team_id = $2",
    )
    .bind(inv_id)
    .bind(team_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn accept_invitation(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let user = sqlx::query_as::<_, crate::db::models::User>(
        "SELECT * FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let invitation = sqlx::query_as::<_, Invitation>(
        "SELECT * FROM invitations WHERE token = $1 AND status = 'pending' AND expires_at > now()",
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Invitation not found or expired".into()))?;

    if invitation.email != user.email {
        return Err(AppError::Forbidden);
    }

    // Add to team
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (team_id, user_id) DO UPDATE SET role = EXCLUDED.role, updated_at = now()",
    )
    .bind(invitation.team_id)
    .bind(user_id)
    .bind(&invitation.role)
    .execute(&state.db)
    .await?;

    // Mark invitation as accepted
    sqlx::query(
        "UPDATE invitations SET status = 'accepted', updated_at = now() WHERE id = $1",
    )
    .bind(invitation.id)
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({ "accepted": true, "team_id": invitation.team_id })))
}

// ---------------------------------------------------------------------------
// Project assignment handlers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TeamProjectResponse {
    project_id: String,
    team_id: String,
    name: String,
    slug: String,
    status: String,
}

async fn list_team_projects(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<TeamProjectResponse>>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    require_org_member(&state, org_id, user_id).await?;

    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .and_then(|c| if c > 0 { Ok(c) } else { Err(sqlx::Error::RowNotFound) })
        .map_err(|_| AppError::NotFound)?;

    #[derive(sqlx::FromRow)]
    struct Row {
        project_id: Uuid,
        team_id: Uuid,
        name: String,
        slug: String,
        status: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT pt.project_id, pt.team_id, p.name, p.slug, p.status
         FROM project_teams pt
         JOIN projects p ON p.id = pt.project_id
         WHERE pt.team_id = $1
         ORDER BY p.name",
    )
    .bind(team_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.iter()
            .map(|r| TeamProjectResponse {
                project_id: r.project_id.to_string(),
                team_id: r.team_id.to_string(),
                name: r.name.clone(),
                slug: r.slug.clone(),
                status: r.status.clone(),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct AssignProjectRequest {
    project_id: Uuid,
}

async fn assign_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((org_id, team_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<AssignProjectRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role = require_team_member(&state, team_id, user_id).await?;
    if !can_manage_members(&role) {
        return Err(AppError::Forbidden);
    }

    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .and_then(|c| if c > 0 { Ok(c) } else { Err(sqlx::Error::RowNotFound) })
        .map_err(|_| AppError::NotFound)?;

    // Verify the project belongs to the caller or to the org
    let proj_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM projects WHERE id = $1 AND user_id = $2 AND status != 'deleted'",
    )
    .bind(req.project_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    if proj_count == 0 {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        "INSERT INTO project_teams (project_id, team_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(req.project_id)
    .bind(team_id)
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({ "assigned": true })))
}

async fn unassign_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((_org_id, team_id, project_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let role = require_team_member(&state, team_id, user_id).await?;
    if !can_manage_members(&role) {
        return Err(AppError::Forbidden);
    }

    sqlx::query("DELETE FROM project_teams WHERE project_id = $1 AND team_id = $2")
        .bind(project_id)
        .bind(team_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn slugify_org(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn generate_token() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    (0..48)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}
