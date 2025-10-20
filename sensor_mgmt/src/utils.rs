use serde::de::{self, Visitor};
use serde::{self, Deserializer};
use std::fmt;
use utoipa::openapi::{Object, ObjectBuilder};

/* ------------------------------------------------ Utopia Specific ------------------------------------------------------------ */

pub fn uuid_schema() -> Object {
    // Utopia doesn't natively support Uuid ...
    ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::String)
        .format(Some(utoipa::openapi::SchemaFormat::Custom(
            "uuid".to_string(),
        )))
        .description(Some("A universally unique identifier (UUID)".to_string()))
        .build()
}

/* ------------------------------------------------ Query Params ------------------------------------------------------------ */

pub const QUERY_PARAM_LIST_SEP: &str = ",";

// Trait to define the conversion logic from query parameter string to object
pub trait QueryParam {
    fn from_query_param(param: String) -> Self;

    fn to_query_param(&self) -> String;
}

pub fn serialize_vec_query_params<T>(list: &Vec<T>) -> String
where
    T: QueryParam,
{
    list.iter()
        .map(|c| c.to_query_param())
        .collect::<Vec<String>>()
        .join(QUERY_PARAM_LIST_SEP)
}

/// Deserializes an optional vec of custom structs separated by "," e.g: cols=A,B,C
pub fn query_param_vec_deserializer<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: QueryParam,
{
    struct StringVecVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T> Visitor<'de> for StringVecVisitor<T>
    where
        T: QueryParam,
    {
        type Value = Option<Vec<T>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string containing a list of objs")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let mut items = Vec::new();

            for param in v.split(QUERY_PARAM_LIST_SEP) {
                items.push(T::from_query_param(param.to_string()));
            }

            Ok(Some(items))
        }
    }

    deserializer.deserialize_any(StringVecVisitor(std::marker::PhantomData::<T>))
}

/* ------------------------------------------------ Error handling ------------------------------------------------------------ */

use actix_web::{body::BoxBody, error::ResponseError};
use actix_web::{http::StatusCode, HttpResponse};
use serde::Serialize;

/// An error that can be propagated even back to the endpoint caller
/// Allows adding context information which is NOT shown to the endpoint caller
/// Can wrap a source error

#[derive(Debug)]
pub enum AppError {
    // The generic error type
    // Using HTTP 500 if no status code is set
    InternalError {
        status: Option<StatusCode>,
        msg: Option<String>,
    },

    // HTTP errors

    // 400
    // Validation {}

    // 403
    Unauthorized {
        msg: Option<String>,
    },

    NotFound {
        msg: Option<String>,
    },

    // Service specific errors
    // Using HTTP 500
    DatabaseError {
        msg: Option<String>,
    },
    // ExternalServiceError {}
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::InternalError { msg, .. } => write!(
                f,
                "Error {}: {}",
                self.status_code().as_u16(),
                msg.clone().unwrap_or("".to_string()),
            ),
            AppError::Unauthorized { msg, .. } => {
                write!(f, "Unauthorized: {}", msg.clone().unwrap_or("".to_string()))
            }
            AppError::NotFound { msg, .. } => {
                write!(f, "Not Found: {}", msg.clone().unwrap_or("".to_string()))
            }
            AppError::DatabaseError { msg, .. } => {
                write!(f, "DB Error: {}", msg.clone().unwrap_or("".to_string()))
            }
        }
    }
}

impl std::error::Error for AppError {}

// Note: The '{context:?}' in #[error(...)] uses Debug format for the context Option.
// You might want a custom Display impl if you want more control over how context is shown in logs.
// If implementing Display manually, you'd match self and format the message and context field.

impl AppError {
    //------------- Constructor -----------------
    pub fn internal<T>(msg: impl Into<String>) -> Result<T, Self> {
        Err(AppError::InternalError {
            status: Some(StatusCode::INTERNAL_SERVER_ERROR),
            msg: Some(msg.into()),
        })
    }

    pub fn db<T>(msg: impl Into<String>) -> Result<T, Self> {
        Err(AppError::InternalError {
            status: Some(StatusCode::INTERNAL_SERVER_ERROR),
            msg: Some(msg.into()),
        })
    }

    pub fn unauthorized(msg: impl Into<String>) -> Result<(), Self> {
        Err(AppError::Unauthorized {
            msg: Some(msg.into()),
        })
    }
    pub fn unauthorized2(msg: impl Into<String>) -> Self {
        AppError::Unauthorized {
            msg: Some(msg.into()),
        }
    }
    pub fn unauthorized_generic() -> Result<(), Self> {
        Err(AppError::Unauthorized { msg: None })
    }
    pub fn unauthorized_generic2() -> Self {
        AppError::Unauthorized { msg: None }
    }

    pub fn not_found(msg: impl Into<String>) -> Result<(), Self> {
        Err(AppError::NotFound {
            msg: Some(msg.into()),
        })
    }
    pub fn not_found2(msg: impl Into<String>) -> Self {
        AppError::NotFound {
            msg: Some(msg.into()),
        }
    }
}

// Define the structure of the JSON response body - public facing
#[derive(Serialize)]
struct ErrorResponseJson {
    // Consider adding a machine-readable error code or type identifier
    // code: String,
    status: u16,
    message: String, // The message shown to the user
    // Include other public details like validation fields if applicable
    #[serde(skip_serializing_if = "Option::is_none")] // Only include if Some
    fields: Option<Vec<String>>,
}

impl actix_web::error::ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        // Map each variant to an appropriate HTTP status code
        match self {
            AppError::InternalError { status, .. } => {
                status.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            }
            AppError::DatabaseError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            AppError::NotFound { .. } => StatusCode::NOT_FOUND,
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let status_code = self.status_code();

        // Determine the user-facing message
        let (user_message, fields) = match self {
            AppError::Unauthorized { msg, .. } => (
                msg.clone()
                    .unwrap_or_else(|| "Unauthorized access".to_string()),
                None,
            ),
            AppError::NotFound { msg, .. } => {
                (msg.clone().unwrap_or_else(|| "Not found".to_string()), None)
            }
            AppError::InternalError { msg, .. } | AppError::DatabaseError { msg, .. } => (
                msg.clone()
                    .unwrap_or_else(|| "Unexpected internal error".to_string()),
                None,
            ),
        };

        // Create the JSON payload struct
        let error_json = ErrorResponseJson {
            status: status_code.as_u16(),
            message: user_message,
            fields, // Include validation fields if present
        };

        // Build the HttpResponse
        // TODO check if encoding header is set correctly
        HttpResponse::build(status_code).json(error_json)
    }
}

// In your enum definition or an impl block, if not using #[from]
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError {
            msg: Some(err.to_string()), // Or map sqlx error to a message
        }
    }
}
impl From<actix_web::Error> for AppError {
    fn from(value: actix_web::Error) -> Self {
        AppError::InternalError {
            status: None,
            msg: Some(value.to_string()),
        }
    }
}
impl From<rumqttc::ClientError> for AppError {
    fn from(value: rumqttc::ClientError) -> Self {
        AppError::InternalError {
            status: None,
            msg: Some(value.to_string()),
        }
    }
}
impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::InternalError {
            status: None,
            msg: Some(value.to_string()),
        }
    }
}
impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        AppError::InternalError {
            status: None,
            msg: Some(value.to_string()),
        }
    }
}
impl Into<HttpResponse> for AppError {
    fn into(self) -> HttpResponse {
        self.error_response()
    }
}

/* ------------------------------------------------ General ------------------------------------------------------------ */

pub fn generate_random_string(len: usize) -> String {
    let charset = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..len)
        .map(|_| {
            let idx = fastrand::usize(..charset.len());
            charset[idx] as char
        })
        .collect()
}
