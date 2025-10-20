use crate::database::data_db::{delete_sensor_data, get_data};
use crate::database::models::db_structs::DBOperation;
use crate::features::cache;
use crate::features::user_sens_perm::UserSensorPerm;
use crate::handler::models::requests::{DataLoadRequestParams, SensorDataDeletionParams};
use crate::handler::{main_hdl, policy};
use crate::state::AppState;
use actix_web::{delete, get, web, Responder};
use chrono::Utc;

/* ------------------------------------------------ Sensor Data ------------------------------------------------------------ */

pub(crate) const COMMON_TAG: &str = "Sensor / Data";

// NOTE:
// HTTP data ingest has been moved to data_ingest/http.rs
// MQTT data ingest is in data_ingest/mqtt.rs

#[utoipa::path(
    delete,
    path = "/api/sensors/{id}/data/delete",
    description = "Deletes entries from the sensor data table for the specified sensor and the time range.<br>\
    Deleting all sensor data by omitting the 'from' and 'to' parameter can only be executed by also specifying the 'purge' flag.",
    params(
        ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string())),
        ("key" = String, Query, description = "The provided API key for writing data.", example = json!(uuid::Uuid::new_v4().to_string())),
        ("from" = Option<String>, Query, description = "Lower border for deleting data entries, RFC3339 without timezone information", example="2006-01-02T15:04:05"),
        ("to" = Option<String>, Query, description = "Upper border for deleting data entries, RFC3339 without timezone information", example="2006-01-02T15:04:05"),
        ("from_inclusive" = Option<bool>, Query, description = "If the 'from' range border should be considered inclusive, default=true"),
        ("to_inclusive" = Option<bool>, Query, description = "If the 'to' range border should be considered inclusive, default=true"),
        ("purge" = Option<bool>, Query, description = "If the entire dataset should be deleted when omitting 'to and 'from' attributes")
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns ok if the deletion was successful."),
        (status = 401, description= "Returns an unauthorized error if access is not permitted."),
        (status = 500, description= "Returns an error if the sensor does not exist or data couldn't be deleted."),
    ),
)]
#[delete("/sensors/{id}/data/delete")]
async fn delete_sensor_data_handler(
    sensor_id: web::Path<uuid::Uuid>,
    params: web::Query<SensorDataDeletionParams>,
    state: web::Data<AppState>,
) -> impl Responder {
    let sensor_id = sensor_id.into_inner();

    // Retrieves the api key if it exists and is valid

    let api_key = match params.key {
        Some(key) => cache::request_api_key(key, &state).await,
        None => None,
    };

    // Verifies key or guest access

    let has_access = match api_key {
        Some(key) => key.sensor_id == sensor_id && key.operation == DBOperation::WRITE,
        None => policy::require_sensor_permission(None, sensor_id, UserSensorPerm::Write, &state)
            .await
            .is_none(),
    };

    if !has_access {
        return policy::unauthorized("No permissions to delete sensor data!".to_string()).unwrap();
    }

    if params.from.is_none()
        && params.to.is_none()
        && (params.purge.is_none() || !params.purge.unwrap())
    {
        return policy::internal_error(
            "Missing 'purge' flag for absent deletion time interval!".to_string(),
        );
    }

    let result = delete_sensor_data(sensor_id, params.into_inner(), &state).await;

    main_hdl::send_result(&result)
}

///
/// NOTE All query parameters must be encoded in a way that enables the serde_urlencoded crate to decode them.
///
#[utoipa::path(
    get,
    path = "/api/sensors/{id}/data/load",
    description = "Retrieves values from the specified sensor based on various predicates.<br>\
    In addition to the data columns, a time column is added to the result rows. By default, this time column is called 'created_at'. \
    For requests with a time_grouping, the time column is called 'grouped_time'.",
    params(
        ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string())),
        ("key" = String, Query, description = "The provided API key for reading data.", example = json!(uuid::Uuid::new_v4().to_string())),
        ("limit" = Option<String>, Query, description = "If this parameter is not present a limit of 10 entries will be assumed as the default.", example = "10"),
        ("ordering" = Option<String>, Query, description = "DESC or ASC", example = "DESC"),
        ("order_col" = Option<String>, Query, description = "The column name to order to result values. By default the time column is used.", example = "col1"),
        ("from" = Option<String>, Query, description = "Lower border for data retrieval, RFC3339 without timezone information", example="2006-01-02T15:04:05"),
        ("to" = Option<String>, Query, description = "Upper border for data retrieval, RFC3339 without timezone information", example="2006-01-02T15:04:05"),
        ("from_inclusive" = Option<bool>, Query, description = "If the 'from' range border should be considered inclusive, default=true"),
        ("to_inclusive" = Option<bool>, Query, description = "If the 'to' range border should be considered inclusive, default=true"),
        ("cols" = Option<String>, Query, description = "Comma separated list of columns to retrieve, e.g. col1,col2<br>\
        In case of time grouping, an aggregation has to be specified for each column, e.g. col1.COUNT,col2.SUM<br>\
        By default, all columns + the time column are retrieved.", example="col1,col2"),
        ("time_grouping" = Option<u32>, Query, description = "Optional time interval in seconds used for grouping the result values.<br>\
        E.g. grouping values in 1 hour intervals (3.600s=1hour).<br>\
        If a time grouping is used, each retrieved column must specify a data aggregation [SUM, COUNT, MAX, MIN, AVG].<br>\
        Result values will contain a 'grouped_time' field defining the grouped time value", example="3600"),
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns the retrieved key/value data.", body=Vec<Value>, example=json!([{"created_at": Utc::now().naive_utc(), "col1": 42, "col2": 51.234, "col3": "Hello"}])),
        (status = 401, description= "Returns an unauthorized error if access is not permitted."),
        (status = 500, description= "Returns an error if the sensor does not exist or the data couldn't be retrieved."),
    ),
)]
#[get("/sensors/{id}/data/load")]
async fn get_sensor_data_handler(
    path: web::Path<uuid::Uuid>,
    params: web::Query<DataLoadRequestParams>,
    state: web::Data<AppState>,
) -> impl Responder {
    let sensor_id = path.into_inner();

    // Retrieves the api key if it exists and is valid

    let api_key = match params.key {
        Some(key) => cache::request_api_key(key, &state).await,
        None => None,
    };

    // Verifies key or guest access

    let has_access = match api_key {
        Some(key) => key.sensor_id == sensor_id && key.operation == DBOperation::READ,
        None => policy::require_sensor_permission(None, sensor_id, UserSensorPerm::Read, &state)
            .await
            .is_none(),
    };

    if !has_access {
        return policy::unauthorized("No permissions to read sensor data!".to_string()).unwrap();
    }

    // By default, we enforce a limit of 10 rows if no limit is set
    let mut query_params = params.into_inner();
    if query_params.limit.is_none() {
        query_params.limit = Some(10);
    }

    let result = get_data(sensor_id, query_params, &state).await;

    main_hdl::send_result(&result)
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::database::data_db::{GROUPED_TIME_COL_NAME, TIME_COL_NAME};
    use crate::database::models::db_structs::{DBAggregation, DBOrdering};
    use crate::features::config::TIMESTAMP_FORMAT;
    use crate::handler::models::requests::DataLoadRequestColumns;
    use crate::handler::models::requests::SensorDataIngestEntry;
    use crate::test_utils::tests::{
        create_test_api_keys, create_test_app, create_test_sensors, execute_request, john,
    };
    use actix_http::body::BoxBody;
    use actix_http::{Method, Request};
    use actix_web::dev::{Service, ServiceResponse};
    use actix_web::http::StatusCode;
    use async_std::task;
    use chrono::{NaiveDateTime, Utc};
    use serde_json::Map;
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_ingest_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let target_sensor_own = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor")
            .unwrap();
        let target_sensor_allowed = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor2")
            .unwrap();
        let target_sensor_not_allowed = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor4")
            .unwrap();
        let public_sensor = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor5")
            .unwrap();

        let payload = json!({
            "col1": 42,
            "col2": 56.789,
            "col3": "Hello",
        });

        let data_entry = SensorDataIngestEntry::from_json(payload, None);

        // --- Ingest allowed (his own) sensor data as john with key - Should succeed ---

        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_own.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                target_sensor_own.1, api_key_write
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Ingest allowed sensor data as john with key - Should succeed ---

        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_allowed.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                target_sensor_allowed.1, api_key_write
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Ingest allowed sensor as john with valid key but wrong op (READ) - Should fail ---

        let api_key_read = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_allowed.1
                    && k.operation == DBOperation::READ
            })
            .unwrap()
            .id;

        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                target_sensor_allowed.1, api_key_read
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Ingest un-allowed sensor with valid key (john) of wrong sensor - Should fail ---

        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                target_sensor_not_allowed.1, api_key_write
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Ingest sensor with invalid key - Should fail ---

        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                target_sensor_not_allowed.1,
                Uuid::new_v4()
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Ingest non-existing sensor without key - Should fail ---

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", Uuid::new_v4()),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // --- Ingest public sensor without key - Should succeed ---

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", public_sensor.1),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Ingest public sensor with valid key of wrong sensor - Should fail ---

        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                public_sensor.1, api_key_write
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Ingest public sensor with invalid key - Should succeed ---

        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                public_sensor.1,
                Uuid::new_v4()
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Ingest various invalid data to public sensor - Should succeed ---

        fn compare_payload(payload: &Value, entry: &Map<String, Value>) -> bool {
            match (payload, entry) {
                (Value::Object(sub_map), super_map) => {
                    sub_map.iter().all(|(k, v)| super_map.get(k) == Some(v))
                }
                _ => false,
            }
        }

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.limit = Some(1);
        req_payload.ordering = Some(DBOrdering::DESC);

        // --- Some columns invalid - should be inserted as NULLs

        let payload = {
            json!({
                "col1": "42",
                "col2": "56.789",
                "col3": "42",
            })
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", public_sensor.1),
            Method::POST,
            None,
            Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check result

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let expected_result = {
            json!({
                "col1": null,
                "col2": null,
                "col3": "42",
            })
        };

        let elm = body.as_array().unwrap().first().unwrap();
        assert!(compare_payload(&expected_result, elm.as_object().unwrap()));

        // --- All columns invalid - should be inserted as NULLs

        let payload = {
            json!({
                "col1": "42",
                "col2": "56.789",
                "col3": 42,
            })
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", public_sensor.1),
            Method::POST,
            None,
            Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check result

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let expected_result = {
            json!({
                "col1": null,
                "col2": null,
                "col3": null,
            })
        };

        let elm = body.as_array().unwrap().first().unwrap();
        assert!(compare_payload(&expected_result, elm.as_object().unwrap()));

        // --- Some invalid col names - Only col2 should be inserted correctly, rest NULL

        let payload = {
            json!({
                "xz": 42,
                "col2": 56.789,
                "bv": 42,
            })
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", public_sensor.1),
            Method::POST,
            None,
            Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check result

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let expected_result = {
            json!({
                "col1": null,
                "col2": 56.789,
                "col3": null,
            })
        };

        let elm = body.as_array().unwrap().first().unwrap();
        assert!(compare_payload(&expected_result, elm.as_object().unwrap()));

        // --- All invalid col names - Insertion should fail

        let payload = {
            json!({
                "xz": 42,
                "gg": 56.789,
                "bv": 42,
            })
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", public_sensor.1),
            Method::POST,
            None,
            Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // --- Insert multiple (valid) values at once | No custom timestamp

        let amount = 10;

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.limit = Some(amount);
        req_payload.ordering = Some(DBOrdering::DESC);

        let mut data_entries: Vec<SensorDataIngestEntry> = Vec::new();

        for _ in 0..amount {
            let payload = json!({
                "col1": 99999,
                "col2": 56.789,
                "col3": "Hello",
            });

            data_entries.push(SensorDataIngestEntry::from_json(payload, None));
        }

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", public_sensor.1),
            Method::POST,
            None,
            Some(data_entries),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check result

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check if tuples are inserted correctly
        let arr = body.as_array().unwrap();

        for elm in arr.iter() {
            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col1")
                    .unwrap()
                    .as_i64()
                    .unwrap(),
                99999
            );
        }

        // Check if timestamp is the same for all tuples (bulk insert) -> checks first and last entry
        let first_created_at = arr
            .first()
            .unwrap()
            .as_object()
            .unwrap()
            .get("created_at")
            .unwrap();
        let last_created_at = arr
            .last()
            .unwrap()
            .as_object()
            .unwrap()
            .get("created_at")
            .unwrap();

        assert_eq!(first_created_at, last_created_at);

        // --- Insert multiple (valid) values at once | Custom timestamp

        let delay_per_tup = 5;

        let mut data_entries: Vec<SensorDataIngestEntry> = Vec::new();

        for i in 0..amount {
            let time: NaiveDateTime =
                Utc::now().naive_utc() + chrono::Duration::milliseconds((i * delay_per_tup) as i64);

            let payload = json!({
                "col1": i,
                "col2": 56.789,
                "col3": "Hello",
            });

            data_entries.push(SensorDataIngestEntry::from_json(payload, Some(time)));
        }

        // Make sure the wait before pushing the data, or it will be rejected by DB (timestamps in future)
        task::sleep(core::time::Duration::from_millis(
            (amount * delay_per_tup) as u64,
        ))
        .await;

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest", public_sensor.1),
            Method::POST,
            None,
            Some(data_entries),
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check result

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check if tuples are inserted correctly and respecting the defined timestamp
        for (index, elm) in body.as_array().unwrap().iter().enumerate() {
            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col1")
                    .unwrap()
                    .as_i64()
                    .unwrap(),
                (amount - 1 - index as i32) as i64
            );
        }
    }

    async fn add_bulk_dummy_data(
        amount: u32,
        delay_per_tup: u32,
        sensor_id: Uuid,
        write_key: Uuid,
        app: &impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
    ) {
        let mut data_entries: Vec<SensorDataIngestEntry> = Vec::new();

        for i in 0..amount {
            let time: NaiveDateTime =
                Utc::now().naive_utc() + chrono::Duration::milliseconds((i * delay_per_tup) as i64);

            let payload = json!({
                "col1": i,
                "col2": 56.789,
                "col3": "Hello",
            });

            data_entries.push(SensorDataIngestEntry::from_json(payload, Some(time)));
        }

        // Make sure the wait before pushing the data, or it will be rejected by DB (timestamps in future)
        task::sleep(core::time::Duration::from_millis(
            (amount * delay_per_tup) as u64,
        ))
        .await;

        let _ = execute_request(
            &format!("/api/sensors/{}/data/ingest?key={}", sensor_id, write_key),
            Method::POST,
            None,
            Some(data_entries),
            None,
            StatusCode::OK,
            &app,
        )
        .await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let target_sensor_own = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor")
            .unwrap();
        let target_sensor_allowed = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor2")
            .unwrap();
        let target_sensor_not_allowed = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor4")
            .unwrap();
        let public_sensor = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor5")
            .unwrap();

        // --- Delete ALL allowed (his own) sensor data as john with key - Should succeed ---

        let api_key_read = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_own.1
                    && k.operation == DBOperation::READ
            })
            .unwrap()
            .id;
        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_own.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        let amount = 10;
        let tup_delay = 100;

        add_bulk_dummy_data(amount, 5, target_sensor_own.1, api_key_write, &app).await;

        let delete_request = SensorDataDeletionParams {
            key: Some(api_key_write),
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(true),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", target_sensor_own.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check if all values were deleted

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        assert_eq!(body.as_array().unwrap().len(), 0);

        // --- Delete SOME [open range] allowed (his own) sensor data as john with key - Should succeed ---

        let time_start: NaiveDateTime = Utc::now().naive_utc();

        add_bulk_dummy_data(amount, tup_delay, target_sensor_own.1, api_key_write, &app).await;

        let delete_threshold: NaiveDateTime =
            time_start + chrono::Duration::milliseconds((amount / 2 * tup_delay) as i64);

        let delete_request = SensorDataDeletionParams {
            key: Some(api_key_write),
            from: Some(delete_threshold),
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(false),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", target_sensor_own.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check results - Get all values and check if only the specified values where deleted! [half values from end]

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.ordering = Some(DBOrdering::DESC);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let res_arr = body.as_array().unwrap();

        assert_eq!(res_arr.len(), (amount / 2) as usize);

        for (index, elm) in body.as_array().unwrap().iter().enumerate() {
            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col1")
                    .unwrap()
                    .as_i64()
                    .unwrap(),
                (amount - 1 - amount / 2 - index as u32) as i64
            );
        }

        // --- Delete SOME [closed range] allowed (his own) sensor data as john with key - Should succeed ---

        let time_start: NaiveDateTime = Utc::now().naive_utc();

        add_bulk_dummy_data(10, tup_delay, target_sensor_own.1, api_key_write, &app).await;

        // to/from are inclusive (here)!
        let delete_to: NaiveDateTime = time_start
            + chrono::Duration::milliseconds(((amount - 2 - 1) * tup_delay) as i64)
            + chrono::Duration::milliseconds((tup_delay / 2) as i64); // Additional delay for consistency

        let delete_request = SensorDataDeletionParams {
            key: Some(api_key_write),
            from: Some(time_start), // Does only remove new values
            to: Some(delete_to),
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(false),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", target_sensor_own.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // Check results - Get all values and check if only the specified values where deleted!

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let res_arr = body.as_array().unwrap();

        assert_eq!(res_arr.len(), 2 + (amount / 2) as usize); // Old and new values

        for (index, elm) in res_arr.iter().enumerate() {
            if index < 2 {
                //New values (starting from last idx)
                assert_eq!(
                    elm.as_object()
                        .unwrap()
                        .get("col1")
                        .unwrap()
                        .as_i64()
                        .unwrap(),
                    (amount - 1 - index as u32) as i64
                );
            } else {
                // Old values starting from amount/2 index
                assert_eq!(
                    elm.as_object()
                        .unwrap()
                        .get("col1")
                        .unwrap()
                        .as_i64()
                        .unwrap(),
                    (amount / 2 - 1 + 2 - index as u32) as i64
                );
            }
        }

        // --- Delete allowed sensor data as john with key - Should succeed ---

        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_allowed.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        let delete_request = SensorDataDeletionParams {
            key: Some(api_key_write),
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(true),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", target_sensor_allowed.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Delete allowed sensor data as john with valid key but wrong op (READ) - Should fail ---

        let api_key_read = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_allowed.1
                    && k.operation == DBOperation::READ
            })
            .unwrap()
            .id;

        let delete_request = SensorDataDeletionParams {
            key: Some(api_key_read),
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(false),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", target_sensor_allowed.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Delete un-allowed sensor data with valid key (john) of wrong sensor - Should fail ---

        let delete_request = SensorDataDeletionParams {
            key: Some(api_key_write),
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(false),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", target_sensor_not_allowed.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Delete sensor data with invalid key - Should fail ---

        let delete_request = SensorDataDeletionParams {
            key: Some(Uuid::new_v4()),
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(false),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", target_sensor_not_allowed.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Delete non-existing sensor data without key - Should fail ---

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", Uuid::new_v4()),
            Method::DELETE,
            Some(SensorDataDeletionParams::default().to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // --- Delete public sensor data without key - Should succeed ---

        let delete_request = SensorDataDeletionParams {
            key: None,
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(true),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", public_sensor.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Delete public sensor data with valid key of wrong sensor - Should fail ---

        let delete_request = SensorDataDeletionParams {
            key: Some(api_key_write),
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(false),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", public_sensor.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Delete public sensor data with invalid key - Should succeed ---

        let delete_request = SensorDataDeletionParams {
            key: Some(Uuid::new_v4()),
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: Some(true),
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", public_sensor.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Delete all public sensor data without purge flag - Should fail ---

        let delete_request = SensorDataDeletionParams {
            key: None,
            from: None,
            to: None,
            to_inclusive: None,
            from_inclusive: None,
            purge: None,
        };

        let _ = execute_request(
            &format!("/api/sensors/{}/data/delete", public_sensor.1),
            Method::DELETE,
            Some(delete_request.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_get_data_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let target_sensor_own = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor")
            .unwrap();
        let target_sensor_allowed = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor2")
            .unwrap();
        let target_sensor_not_allowed = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor4")
            .unwrap();
        let public_sensor = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor5")
            .unwrap();

        let mut req_payload = DataLoadRequestParams::default();

        fn compare_payload(payload: &Value, entry: &Map<String, Value>) -> bool {
            let epsilon = 1e-5;

            fn compare_floats_with_epsilon(a: f64, b: f64, epsilon: f64) -> bool {
                (a - b).abs() < epsilon
            }

            match (payload, entry) {
                // When payload is an Object (Map), compare recursively
                (Value::Object(sub_map), super_map) => {
                    sub_map.iter().all(|(k, v)| {
                        if let Some(super_val) = super_map.get(k) {
                            // Compare as floats with epsilon if both are f64
                            if let (Value::Number(v_num), Value::Number(super_num)) = (v, super_val)
                            {
                                if let (Some(v_float), Some(super_float)) =
                                    (v_num.as_f64(), super_num.as_f64())
                                {
                                    return compare_floats_with_epsilon(
                                        v_float,
                                        super_float,
                                        epsilon,
                                    );
                                }
                            }
                            // Otherwise, compare directly (e.g., strings, booleans, etc.)
                            return super_val == v;
                        }
                        false // If the key is not found, return false
                    })
                }
                // If it's not an object, just return false
                _ => false,
            }
        }

        // --- Ingest allowed (his own) sensor dummy data as john - Should succeed ---

        // Ingest 10 dummy data entries

        let mut payloads: Vec<Value> = Vec::new();

        for i in 0..10 {
            let payload = {
                json!({
                    "col1": i,
                    "col2": 1.11 * i as f64,
                    "col3": format!("Hallo_{}", "o".repeat(10 - i)),
                })
            };

            payloads.push(payload);
        }

        let time_start: NaiveDateTime = Utc::now().naive_utc();

        let mut time_between: NaiveDateTime = Default::default();

        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_own.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        for (index, payload) in payloads.iter().enumerate() {
            let entry = SensorDataIngestEntry::from_json(payload.clone(), None);

            execute_request(
                &format!("/api/sensors/{}/data/ingest", target_sensor_own.1),
                Method::POST,
                Some(vec![("key".to_string(), api_key_write.to_string())]),
                Some(vec![entry]),
                None,
                StatusCode::OK,
                &app,
            )
            .await;

            task::sleep(core::time::Duration::from_millis(50)).await; // A little bit of time for time tracking

            if index == 4 {
                time_between = Utc::now().naive_utc() + core::time::Duration::from_millis(100); // Inconsistent rounding issues in db?

                task::sleep(core::time::Duration::from_millis(200)).await; // More sleep to make test more consistent
            }
        }

        let time_end: NaiveDateTime = Utc::now().naive_utc();

        // --- Get own sensor data with valid key (john) - Should succeed ---

        let api_key_read = test_keys
            .iter()
            .find(|key| {
                key.sensor_id == target_sensor_own.1
                    && key.user_id == john().id
                    && key.operation == DBOperation::READ
            })
            .unwrap()
            .id;
        req_payload.key = Some(api_key_read);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), payloads.len()); // We retrieved all data

        // Check if data is correct

        for (index, elm) in elms.iter().enumerate() {
            assert!(compare_payload(
                payloads.get(index).unwrap(),
                elm.as_object().unwrap()
            ));
        }

        // --- Perform various allowed data retrievals to check correctness of data ---

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.limit = Some(5);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        assert_eq!(body.as_array().unwrap().len(), 5);

        // Get the first (oldest) element

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.limit = Some(1);
        req_payload.ordering = Some(DBOrdering::ASC);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        assert!(compare_payload(
            payloads.first().unwrap(),
            body.as_array()
                .unwrap()
                .first()
                .unwrap()
                .as_object()
                .unwrap()
        ));

        // Get the last (newest) element

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.limit = Some(1);
        req_payload.ordering = Some(DBOrdering::DESC);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        assert!(compare_payload(
            payloads.last().unwrap(),
            body.as_array()
                .unwrap()
                .first()
                .unwrap()
                .as_object()
                .unwrap()
        ));

        // Get last elements in custom order [col3]

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.limit = Some(1);
        req_payload.ordering = Some(DBOrdering::DESC);
        req_payload.order_col = Some("col3".to_string());

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        assert!(compare_payload(
            payloads.first().unwrap(),
            body.as_array()
                .unwrap()
                .first()
                .unwrap()
                .as_object()
                .unwrap()
        ));

        // Get the first 5 elements based on time

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.from = Some(time_start);
        req_payload.to = Some(time_between);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), 5);

        for (index, elm) in elms.iter().enumerate() {
            assert!(compare_payload(
                payloads.get(index).unwrap(),
                elm.as_object().unwrap()
            ));
        }

        // Get the last 5 elements based on time
        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.from = Some(time_between);
        req_payload.to = Some(time_end);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), 5);

        for (index, elm) in elms.iter().enumerate() {
            // 5 last elements are ASC sorted
            assert!(compare_payload(
                payloads.get(5 + index).unwrap(),
                elm.as_object().unwrap()
            ));
        }

        // All together, last 3 elements (of 5) desc

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.limit = Some(3);
        req_payload.ordering = Some(DBOrdering::DESC);
        req_payload.from = Some(time_between);
        req_payload.to = Some(time_end);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), 3);

        for (index, elm) in elms.iter().enumerate() {
            assert!(compare_payload(
                payloads.get(9 - index).unwrap(),
                elm.as_object().unwrap()
            ));
        }

        // Only return col1 as result (+time)

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col1".to_string(),
            aggregation: None,
        }]);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        for elm in body.as_array().unwrap().iter() {
            assert!(
                elm.as_object().unwrap().contains_key("col1")
                    && elm.as_object().unwrap().contains_key(TIME_COL_NAME)
                    && elm.as_object().unwrap().len() == 2
            );
        }

        // Return invalid col -> Error

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col5".to_string(),
            aggregation: None,
        }]);

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // Return no cols -> Error

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.cols = Some(Vec::new());

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // Return aggregated col but not specifying time grouping -> Error

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col1".to_string(),
            aggregation: Some(DBAggregation::SUM),
        }]);

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // Specify time grouping but no aggregated cols -> Error

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.time_grouping = Some(1);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col1".to_string(),
            aggregation: None,
        }]);

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // Specify time grouping but no aggregated cols (default cols) -> Error

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.time_grouping = Some(1);

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // Mixed aggregated and non-aggregated cols -> Error

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.cols = Some(vec![
            DataLoadRequestColumns {
                name: "col1".to_string(),
                aggregation: Some(DBAggregation::SUM),
            },
            DataLoadRequestColumns {
                name: "col2".to_string(),
                aggregation: None,
            },
        ]);

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // Invalid aggregations -> Error

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col3".to_string(),
            aggregation: Some(DBAggregation::SUM),
        }]);

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col3".to_string(),
            aggregation: Some(DBAggregation::AVG),
        }]);

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // Only return col1 aggregated + time col

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.time_grouping = Some(100);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col1".to_string(),
            aggregation: Some(DBAggregation::SUM),
        }]);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let elm = body.as_array().unwrap();

        let col1_sum: i64 = payloads
            .iter()
            .filter_map(|item| item.get("col1").and_then(Value::as_i64))
            .sum();

        assert_eq!(elm.len(), 1);
        assert_eq!(
            elm.first()
                .unwrap()
                .as_object()
                .unwrap()
                .get("col1")
                .unwrap()
                .as_i64()
                .unwrap(),
            col1_sum
        );

        // Test various aggregations

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.time_grouping = Some(100);
        req_payload.cols = Some(vec![
            DataLoadRequestColumns {
                name: "col1".to_string(),
                aggregation: Some(DBAggregation::MIN),
            },
            DataLoadRequestColumns {
                name: "col2".to_string(),
                aggregation: Some(DBAggregation::MIN),
            },
            DataLoadRequestColumns {
                name: "col3".to_string(),
                aggregation: Some(DBAggregation::MIN),
            },
        ]);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let elm = body
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .as_object()
            .unwrap();

        let col1_min: i64 = payloads
            .iter()
            .filter_map(|item| item.get("col1").and_then(Value::as_i64))
            .min()
            .unwrap();
        let col2_min: f64 = payloads
            .iter()
            .filter_map(|item| item.get("col2").and_then(Value::as_f64))
            .filter(|&val| !val.is_nan()) // Exclude NaN values
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Greater))
            .unwrap();
        let col3_min: String = payloads
            .iter()
            .filter_map(|item| item.get("col3").and_then(Value::as_str))
            .map(|s| s.to_string())
            .min()
            .unwrap();

        assert_eq!(elm.get("col1").unwrap().as_i64().unwrap(), col1_min);
        assert_eq!(elm.get("col2").unwrap().as_f64().unwrap(), col2_min);
        assert_eq!(elm.get("col3").unwrap().as_str().unwrap(), col3_min);

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.time_grouping = Some(100);
        req_payload.cols = Some(vec![
            DataLoadRequestColumns {
                name: "col1".to_string(),
                aggregation: Some(DBAggregation::AVG),
            },
            DataLoadRequestColumns {
                name: "col2".to_string(),
                aggregation: Some(DBAggregation::SUM),
            },
            DataLoadRequestColumns {
                name: "col3".to_string(),
                aggregation: Some(DBAggregation::COUNT),
            },
        ]);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let elm = body
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .as_object()
            .unwrap();

        let (sum, count): (f64, i32) = payloads
            .iter()
            .filter_map(|item| item.get("col1").and_then(Value::as_f64))
            .fold((0.0, 0), |(sum, count), val| (sum + val, count + 1));
        let col1_avg = sum / count as f64;
        let (col2_sum, _): (f64, i32) = payloads
            .iter()
            .filter_map(|item| item.get("col2").and_then(Value::as_f64))
            .fold((0.0, 0), |(sum, count), val| (sum + val, count + 1));
        let col3_count = payloads.len();

        assert_eq!(elm.get("col1").unwrap().as_f64().unwrap(), col1_avg);
        assert_eq!(elm.get("col2").unwrap().as_f64().unwrap(), col2_sum);
        assert_eq!(
            elm.get("col3").unwrap().as_i64().unwrap(),
            col3_count as i64
        );

        // Test multiple time buckets

        // Insert some more values after a delay to test multiple buckets
        task::sleep(core::time::Duration::from_millis(1500)).await;

        for payload in payloads.iter() {
            let entry = SensorDataIngestEntry::from_json(payload.clone(), None);

            execute_request(
                &format!("/api/sensors/{}/data/ingest", target_sensor_own.1),
                Method::POST,
                Some(vec![("key".to_string(), api_key_write.to_string())]),
                Some(vec![entry]),
                None,
                StatusCode::OK,
                &app,
            )
            .await;
        }

        // First, get all data to check expected bucket count

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.limit = Some(100);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let full_data = body.as_array().unwrap();

        let mut buckets = HashMap::new();
        let mut half_buckets = HashMap::new(); // Counts only half of the values before second insert

        let half_threshold = time_end.and_utc().timestamp();

        fn json_timestamp_to_epoch(data: &Value) -> i64 {
            let timestamp = data.as_str().unwrap();
            let time = NaiveDateTime::parse_from_str(timestamp, TIMESTAMP_FORMAT).unwrap();

            let epoch = time.and_utc().timestamp();

            epoch
        }

        for data in full_data.iter() {
            let epoch =
                json_timestamp_to_epoch(data.as_object().unwrap().get("created_at").unwrap());

            *buckets.entry(epoch).or_insert(0) += 1;

            if epoch > half_threshold {
                continue;
            }

            *half_buckets.entry(epoch).or_insert(0) += 1;
        }

        // Now get aggregated data

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.ordering = Some(DBOrdering::DESC);
        req_payload.order_col = Some("col1".to_string());
        req_payload.time_grouping = Some(1);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col1".to_string(),
            aggregation: Some(DBAggregation::COUNT),
        }]);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let arr = body.as_array().unwrap();

        assert_eq!(arr.len(), buckets.len()); // Check bucket count

        // Check if ordering is correct (DESC for col1)
        assert_eq!(
            arr.last()
                .unwrap()
                .as_object()
                .unwrap()
                .get("col1")
                .unwrap(),
            buckets
                .iter()
                .min_by_key(|entry| entry.1)
                .unwrap()
                .1
                .clone()
        );

        // Check if correct number of elements where counted across all buckets
        let mut total_elms = 0;

        for elm in arr.iter() {
            let obj = elm.as_object().unwrap();
            let count = obj.get("col1").unwrap().as_i64().unwrap();

            total_elms += count;

            let epoch = json_timestamp_to_epoch(obj.get(GROUPED_TIME_COL_NAME).unwrap());

            assert_eq!(buckets.get(&epoch).unwrap().clone(), count);
        }

        assert_eq!(total_elms, 2 * payloads.len() as i64);

        // Check if time predicate still works with aggregation [only consider half of the values]

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(api_key_read);
        req_payload.to = Some(time_end); // This contains only half of the values
        req_payload.ordering = Some(DBOrdering::DESC);
        req_payload.time_grouping = Some(1);
        req_payload.cols = Some(vec![DataLoadRequestColumns {
            name: "col1".to_string(),
            aggregation: Some(DBAggregation::COUNT),
        }]);

        let body = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_own.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let arr = body.as_array().unwrap();

        assert_eq!(arr.len(), half_buckets.len()); // Check (half) bucket count
                                                   // Check if ordering is correct [DESC for time]
        assert_eq!(
            json_timestamp_to_epoch(
                arr.last()
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .get(GROUPED_TIME_COL_NAME)
                    .unwrap()
            ),
            half_buckets
                .iter()
                .min_by_key(|entry| entry.0)
                .unwrap()
                .0
                .clone()
        );

        // --- Get allowed sensor data with valid key (john) - Should succeed ---

        let mut req_payload = DataLoadRequestParams::default();
        req_payload.key = Some(
            test_keys
                .iter()
                .find(|k| {
                    k.user_id == john().id
                        && k.sensor_id == target_sensor_allowed.1
                        && k.operation == DBOperation::READ
                })
                .unwrap()
                .id,
        );

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_allowed.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Get not allowed sensor data with valid key (john) of wrong sensor - Should fail ---

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_not_allowed.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Get allowed sensor data with valid key (john) but wrong op (write) - Should fail ---

        req_payload.key = Some(
            test_keys
                .iter()
                .find(|k| {
                    k.user_id == john().id
                        && k.sensor_id == target_sensor_allowed.1
                        && k.operation == DBOperation::WRITE
                })
                .unwrap()
                .id,
        );

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_allowed.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Get non-existing sensor data without key - Should fail ---

        req_payload.key = None;
        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", Uuid::new_v4()),
            Method::GET,
            Some(DataLoadRequestParams::default().to_vector()),
            None::<Value>,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // --- Get non-public sensor data without key - Should fail ---

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", target_sensor_not_allowed.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Get public sensor data without key - Should succeed ---

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Get public sensor data with valid key of wrong sensor - Should fail ---

        req_payload.key = Some(
            test_keys
                .iter()
                .find(|k| {
                    k.user_id == john().id
                        && k.sensor_id == target_sensor_allowed.1
                        && k.operation == DBOperation::WRITE
                })
                .unwrap()
                .id,
        );

        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Get public sensor data with invalid key - Should succeed ---

        req_payload.key = Some(Uuid::new_v4());
        let _ = execute_request(
            &format!("/api/sensors/{}/data/load", public_sensor.1),
            Method::GET,
            Some(req_payload.to_vector()),
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;
    }
}
