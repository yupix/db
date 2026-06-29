use crate::config::Config;
use bollard::Docker;
use sqlx::PgPool;

pub struct AppState {
    pub db: PgPool,
    pub docker: Docker,
    pub config: Config,
}
