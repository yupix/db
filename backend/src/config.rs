#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    /// Host-side directory backup archives are written to. Must survive
    /// project/container deletion, so it must NOT be inside a project
    /// container's own filesystem.
    pub backup_dir: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://admin:admin123@localhost:5432/dbcontrol".into()),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-me".into()),
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()?,
            backup_dir: std::env::var("BACKUP_DIR").unwrap_or_else(|_| "./data/backups".into()),
        })
    }
}
