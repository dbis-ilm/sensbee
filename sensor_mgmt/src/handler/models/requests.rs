use crate::utils::uuid_schema;
use serde_derive::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::database::models::db_structs::{DBOperation, DBOrdering};
use crate::database::models::sensor::SensorColumn;
use crate::features::sensor_data_storage::SensorDataStorageCfg;

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone)]
pub struct ApiKeyQueryParam {
    #[schema(schema_with = uuid_schema)]
    pub key: Option<uuid::Uuid>
}

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone)]
pub struct CreateSensorRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<(f64,f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub permissions: Vec<SensorPermissionRequest>,
    pub columns: Vec<SensorColumn>,
    pub storage: SensorDataStorageCfg,
    // TODO: Later we could specify an ingest method (http, mqtt, ...)
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct EditSensorRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<(f64,f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub permissions: Vec<SensorPermissionRequest>,
    pub storage: SensorDataStorageCfg,
    // TODO: Later we could update the ingest method (http, mqtt, ...)
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SensorPermissionRequest {
    pub role_name: String,
    pub operations: Vec<DBOperation>,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct SensorDataRequest {
    pub limit: Option<i32>,
    pub ordering: Option<DBOrdering>,
    // NaiveDateTime is parsed with a trailing "Z" for the TimeZone which cant be parsed by 
    // serde_json since NaiveDateTime does not contain a timezone!
    #[schema(example="01.12.1970T12:00:00")]
    pub from: Option<chrono::NaiveDateTime>,
    #[schema(example="01.12.1970T12:00:00")]
    pub to: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterUserRequest {
    pub name: String,
    pub email: String,
    pub password: String
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginUserRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub operation: DBOperation,
}