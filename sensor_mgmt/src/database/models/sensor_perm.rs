use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::utils::uuid_schema;
use utoipa::ToSchema;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone, ToSchema)]
pub struct SensorPermission {
    #[schema(schema_with = uuid_schema)]
    pub sensor_id: Uuid,
    pub role_id: i32,
    pub allow_info: bool,
    pub allow_read: bool,
    pub allow_write: bool,
}
