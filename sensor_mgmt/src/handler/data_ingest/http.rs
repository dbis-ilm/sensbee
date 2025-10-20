use actix_web::{post, web, HttpResponse, Responder};
use chrono::Utc;
use serde_json::json;
use crate::handler::data_hdl;
use crate::handler::data_ingest::ingest::ingest_data_buisness_logic;
use crate::handler::models::requests::{SensorDataIngestEntry, DataIngestRequestParams};
use crate::state::AppState;


/* ------------------------------------------------Data Management ------------------------------------------------------------ */

#[utoipa::path(
    post,
    path = "/api/sensors/{id}/data/ingest",
    request_body(
        content_type = "application/json",
        content = Vec<SensorDataIngestEntry>,
        description = "Data entries with column names and values to insert for the specified sensor.<br>\
        If invalid data is provided for the columns, NULLs will be inserted. The timestamp of the data tuple \
        may be provided in ISO 8601 format. Timestamps in the future will be rejected.<br>\
        Care: Inserting multiple values without specifying a custom timestamp will result in the same timestamp for all entries.",
        example = json!([{"timestamp": Utc::now().naive_utc(), "col1": 1, "col2": 4.21, "col3": "hello"}])
    ),
    params( 
        ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string())),
        ("key" = String, Query, description = "The provided API key for writing data.", example = json!(uuid::Uuid::new_v4().to_string()))
    ),
    tag = data_hdl::COMMON_TAG,
    responses(
        (status = 200, description = "Returns OK if the insertion into the DB was successful."),
        (status = 204, description = "Returns NO_CONTENT if the entry didnt produce an insertion into the DB but also didnt produce an error."),
        (status = 400, description = "Returns the BAD_REQUEST status if the input parameters are malformed."),
        (status = 401, description= "Returns the unauthorized status if access is not permitted."),
        (status = 500, description= "Returns the generic error status if something unexpected went wrong"),
    ),
)]

#[post("/sensors/{id}/data/ingest")]
async fn ingest_sensor_data_handler(sensor_id: web::Path<uuid::Uuid>, data: web::Bytes, params: web::Query<DataIngestRequestParams>, state: web::Data<AppState>) -> impl Responder  {

    let res = ingest_data_buisness_logic(sensor_id.into_inner(), params.key, data, &state).await;
    let r: HttpResponse = match res {
        Err(err) => err.into(),
        Ok(r) => {
            match r {
                true => HttpResponse::Ok().json(json!({})),
                false => HttpResponse::NoContent().finish(),
            }
        },
    };
    r
}