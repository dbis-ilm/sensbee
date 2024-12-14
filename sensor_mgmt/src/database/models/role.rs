use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const ROLE_SYSTEM_ADMIN: &str = "Admin";
pub const ROLE_SYSTEM_USER: &str = "User";
pub const ROLE_SYSTEM_GUEST: &str = "Guest";

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Clone, ToSchema)]
pub struct Role {
    pub id: i32,
    pub name: String,
    pub system: bool,
}

impl Role {
    pub fn is_admin(&self) -> bool {
        self.name.eq(ROLE_SYSTEM_ADMIN)
    }
}
