use crate::utils::uuid_schema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use crate::database::models::role::Role;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub password: String,
    pub verified: bool,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct UserInfo {
    #[schema(schema_with = uuid_schema)]
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub verified: bool,
    pub roles: Vec<Role>,
}