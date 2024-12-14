use sqlx::{Error, Postgres};
use sqlx::pool::PoolConnection;

#[derive(Debug, Clone)]
pub struct JWTConfig {
    pub private_key: String,
    pub public_key: String,
    pub max_age: i64
}

impl JWTConfig {
    pub fn init() -> JWTConfig {
        let jwt_private_key = std::env::var("ACCESS_TOKEN_PRIVATE_KEY").expect("ACCESS_TOKEN_PRIVATE_KEY must be set");
        let jwt_public_key = std::env::var("ACCESS_TOKEN_PUBLIC_KEY").expect("ACCESS_TOKEN_PUBLIC_KEY must be set");
        let jwt_max_age = std::env::var("ACCESS_TOKEN_MAX_AGE").expect("ACCESS_TOKEN_MAX_AGE must be set");

        JWTConfig {
            private_key: jwt_private_key,
            public_key: jwt_public_key,
            max_age: jwt_max_age.parse::<i64>().unwrap(),
        }
    }
}

/// Application state shared across the system.
/// Safe to clone since PgPool only clones the reference to the underlying db pool.
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt: JWTConfig,
}

impl AppState {
    pub async fn get_db_connection(&self) -> Result<PoolConnection<Postgres>, Error> {
        let r = self.db.acquire().await;

        r
    }
}
