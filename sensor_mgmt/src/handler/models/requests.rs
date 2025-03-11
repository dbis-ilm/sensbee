use crate::utils::uuid_schema;
use serde_derive::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::database::models::db_structs::{DBOperation, DBOrdering};
use crate::database::models::sensor::SensorColumn;
use crate::features::sensor_data_storage::SensorDataStorageCfg;

/// Query Parameters
/// 
/// The timestamp uses %Y-%m-%dT%H:%M:%S%.3f as the format in the actual db query 
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, Default)]
pub struct DataLoadRequestParams {
    /// The uuid of the key that sould be used to verify the authorization to load the data
    /// Leave empty if the data is publicly available
    #[schema(schema_with = uuid_schema)]
    pub key: Option<uuid::Uuid>,
    /// How many rows to return
    pub limit: Option<i32>,
    pub ordering: Option<DBOrdering>,
    // NaiveDateTime is parsed with a trailing "Z" for the TimeZone which cant be parsed by 
    // serde_json since NaiveDateTime does not contain a timezone!
    /// ISO 8601 timestamp
    #[schema(example="2025-02-11T08:27:17")]
    pub from: Option<chrono::NaiveDateTime>,
    /// ISO 8601 timestamp
    #[schema(example="2025-02-11T08:27:17")]
    pub to: Option<chrono::NaiveDateTime>,

    // HINTS
    // these are intended to let the handler know what to do but might get ignored later on
    // Which columns and how to aggregate those columns
    //pub col: Option<Vec<SensorDataRequestColumn>>,
}

impl DataLoadRequestParams {
    /// Method to convert struct fields into a vector when used for query params
    pub fn to_vector(&self) -> Vec<(String, String)> {
        let mut vec = Vec::new();

        if let Some(v) = &self.key {
            vec.push(("key".to_string(), v.to_string()));
        }

        if let Some(v) = &self.limit {
            vec.push(("limit".to_string(), v.to_string()));
        }

        if let Some(v) = &self.ordering {
            vec.push(("ordering".to_string(), v.to_string()));
        }

        if let Some(v) = &self.from {
            vec.push(("from".to_string(), v.format(crate::utils::TIMESTAMP_FORMAT).to_string()));
        }

        if let Some(v) = &self.to {
            vec.push(("to".to_string(), v.format(crate::utils::TIMESTAMP_FORMAT).to_string()));
        }

        vec
    }
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
    #[schema(schema_with = uuid_schema)]
    pub role_id: uuid::Uuid,
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

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct EditUserInfoRequest {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct EditUserPasswordRequest {
    pub old: String,
    pub new: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct CreateRoleRequest {
    pub name: String,
}