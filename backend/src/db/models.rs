use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Team {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct TeamMember {
    pub id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Backup {
    pub id: Uuid,
    pub project_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub file_path: String,
    pub size_bytes: Option<i64>,
    pub status: String,
    pub kind: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct BackupPolicy {
    pub project_id: Uuid,
    pub enabled: bool,
    pub schedule_hour: i32,
    pub daily_keep: i32,
    pub weekly_keep: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct MetricAlert {
    pub id: Uuid,
    pub project_id: Uuid,
    pub metric: String,
    pub comparison: String,
    pub threshold: f64,
    pub enabled: bool,
    pub triggered: bool,
    pub last_triggered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Invitation {
    pub id: Uuid,
    pub team_id: Uuid,
    pub email: String,
    pub role: String,
    pub token: String,
    pub status: String,
    pub invited_by: Uuid,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Project {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub slug: String,
    pub container_id: Option<String>,
    pub container_name: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub port: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub pgbouncer_container_id: Option<String>,
    pub pgbouncer_port: Option<i32>,
    pub docker_network_id: Option<String>,
    pub pool_mode: String,
    pub max_client_conn: i32,
    pub default_pool_size: i32,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct ProjectEnvironment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub endpoint_type: String,
    pub connection_string: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Branch {
    pub id: Uuid,
    pub project_id: Uuid,
    pub parent_branch_id: Option<Uuid>,
    pub name: String,
    pub container_id: Option<String>,
    pub container_name: String,
    pub port: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
