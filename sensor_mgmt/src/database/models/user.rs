use crate::database::models::role::Role;
use crate::utils::uuid_schema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub verified: bool,
}

#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct UserOnlyId {
    pub id: Uuid,
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
