use utoipa::openapi::{Object, ObjectBuilder};

pub static TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f";

/* ------------------------------------------------ Utopia Specific ------------------------------------------------------------ */

pub fn uuid_schema() -> Object {
    // Utopia doesn't natively support Uuid ...
    ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::String)
        .format(Some(utoipa::openapi::SchemaFormat::Custom("uuid".to_string())))
        .description(Some("A universally unique identifier (UUID)".to_string()))
        .build()
}

use derive_more::derive::{Display,Error};
use actix_web::{HttpResponse, http::StatusCode};
use serde::Serialize;
use actix_web::error;
use actix_web::http::header::ContentType;

/* ------------------------------------------------ Error handling ------------------------------------------------------------ */

#[derive(Debug, Display, Error, Serialize)]
pub enum SBError {
    #[display("UNAUTHORIZED")]
    Unauthorized,
}

impl error::ResponseError for SBError {
    fn status_code(&self) -> StatusCode {
        match *self {
            SBError::Unauthorized => StatusCode::UNAUTHORIZED,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // Converts
        let body = serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_string());
        // TODO allow custom message?
        // ...
        // Build response
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::json())
            .body(body)
    }
}