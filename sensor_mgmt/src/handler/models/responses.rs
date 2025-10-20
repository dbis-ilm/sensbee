use crate::database::models::sensor::FullSensorInfo;
use crate::features::user_sens_perm::UserSensorPermissions;
use crate::{database::models::api_key::ApiKey, utils::uuid_schema};
use openidconnect::{AuthorizationCode, CsrfToken, PkceCodeVerifier};
use serde_derive::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SensorDetailResponse {
    pub sensor_info: FullSensorInfo,
    #[serde(flatten)]
    pub user_permissions: UserSensorPermissions,
    pub api_keys: Vec<ApiKey>,
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
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

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct CreateDataTransformScriptResponse {
    pub script: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema, FromRow)]
pub struct ListTransformScriptResponseElem {
    #[schema(schema_with = uuid_schema)]
    pub script_id: Uuid,
    pub name: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema, FromRow)]
pub struct ListEventHandlerResponseElem {
    #[schema(schema_with = uuid_schema)]
    pub id: Uuid,
    pub name: String,
}

// ------
// OpenID

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum AuthResponse {
    Success {
        code: AuthorizationCode,
        state: CsrfToken,
    },
    Error {
        error: String,
        error_description: Option<String>,
        state: CsrfToken,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthSessionData {
    pub pkce_verifier: PkceCodeVerifier,
    pub csrf_token: CsrfToken,
    pub id_token: String,
}
