use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::database::models::db_structs::DBOperation;
use crate::utils::uuid_schema;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone, ToSchema)]
pub struct ApiKey {
    #[schema(schema_with = uuid_schema)]
    pub id: uuid::Uuid,
    #[schema(schema_with = uuid_schema)]
    pub user_id: uuid::Uuid,
    #[schema(schema_with = uuid_schema)]
    pub sensor_id: uuid::Uuid,
    pub name: String,
    pub operation: DBOperation
}
