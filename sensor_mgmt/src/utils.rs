use utoipa::openapi::{Object, ObjectBuilder};

/* ------------------------------------------------ Utopia Specific ------------------------------------------------------------ */

pub fn uuid_schema() -> Object {
    // Utopia doesn't natively support Uuid ...
    ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::String)
        .format(Some(utoipa::openapi::SchemaFormat::Custom("uuid".to_string())))
        .description(Some("A universally unique identifier (UUID)".to_string()))
        .build()
}
