use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::utils::uuid_schema;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, FromRow)]
pub struct DataTransformer {
    #[schema(schema_with = uuid_schema)]
    pub id: uuid::Uuid,
    pub name: String,
    #[sqlx(default)]
    pub script: String,

    // TODO remove options, and version should be u
    pub created_at: NaiveDateTime,         // Timestamp of the creation
    pub version: i32,                      // An incremental counter
    pub updated_at: Option<NaiveDateTime>, // Timestamp of the last chnage to this entry
}
