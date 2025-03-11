use serde_derive::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::fmt;
use crate::database::models::api_key::ApiKey;
use crate::database::models::sensor::FullSensorInfo;
use crate::features::user_sens_perm::UserSensorPermissions;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SensorDetailResponse {
    pub sensor_info: FullSensorInfo,
    #[serde(flatten)]
    pub user_permissions: UserSensorPermissions,
    pub api_keys: Vec<ApiKey>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct GenericUuidResponse {
    pub uuid: String, // NOTE This should always be a Uuid but that type cant be used for OpenAPI Doc generation
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct LoginResponse {
    pub jwt: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub message: String,
}

impl fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(&self).unwrap())
    }
}