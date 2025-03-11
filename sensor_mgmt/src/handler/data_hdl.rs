use actix_web::{get, post, web, HttpResponse, Responder};
use chrono::Utc;
use serde_json::{json, Value};
use crate::database::models::db_structs::DBOperation;
use crate::database::sensor_db::{add_sensor_data, get_data};
use crate::handler::{main_hdl, policy};
use crate::features::cache;
use crate::handler::policy::unauthorized;
use crate::handler::models::requests::DataLoadRequestParams;
use crate::features::user_sens_perm::UserSensorPerm;
use crate::state::AppState;

/* ------------------------------------------------Data Management ------------------------------------------------------------ */

// api/data/sensor/{id}/[ingest|load] 

#[utoipa::path(
    post,
    path = "/api/data/sensor/{id}/ingest",
    request_body(
        content_type = "application/json",
        description = "Key/Value object with column names and values to insert for the specified sensor.<br>\
        If invalid data is provided for the columns, NULLs will be inserted.",
        example = json!({"col1": 42, "col2": 51.234, "col3": "Hello"})
    ),
    params( 
        ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string())),
        ("key" = String, Query, description = "The provided API key for writing data.", example = json!(uuid::Uuid::new_v4().to_string()))
    ),
    tag = "Data",
    responses(
        (status = 200, description = "Returns ok if the insertion was successful."),
        (status = 401, description= "Returns an unauthorized error if access is not permitted."),
        (status = 500, description= "Returns an error if the sensor does not exist or data couldn't be inserted."),
    ),
)]

#[post("/data/sensor/{id}/ingest")]
async fn ingest_sensor_data_handler(sensor_id: web::Path<uuid::Uuid>, params: web::Query<DataLoadRequestParams>, body: web::Bytes, state: web::Data<AppState>) -> impl Responder {
    match serde_json::from_slice::<Value>(&body) {
        Ok(v) => {
            let sensor_id = sensor_id.into_inner();

            // Retrieves the api key if it exists and is valid

            let api_key = match params.key {
                Some(key) => cache::request_api_key(key, &state).await,
                None => None
            };

            // Verifies key or guest access

            let has_access = match api_key {
                Some(key) => key.sensor_id == sensor_id && key.operation == DBOperation::WRITE,
                None => policy::require_sensor_permission(None, sensor_id, UserSensorPerm::Write, &state).await.is_none()
            };

            if !has_access {
                return unauthorized("No permissions to write sensor data!".to_string()).unwrap();
            }

            let result = add_sensor_data(sensor_id, &v, &state).await;

            main_hdl::send_result(&result)
        }
        
        Err(e) => {
            println!("{}", format!("{:?}", e));
            HttpResponse::InternalServerError().json(json!({"error": format!("{:?}", e)}))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/data/sensor/{id}/load",
    params(
        ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string())),
        ("key" = String, Query, description = "The provided API key for reading data.", example = json!(uuid::Uuid::new_v4().to_string())),
        ("limit" = String, Query, description = "If this parameter is not present a limit of 10 entries will be assumed as the default.", example = "10"),
        ("ordering" = String, Query, description = "DESC or ASC", example = "DESC"),
        ("from" = String, Query, description = "RFC3339 without timezone information", example="2006-01-02T15:04:05"),
        ("to" = String, Query, description = "RFC3339 without timezone information", example="2006-01-02T15:04:05"),
    ),
    tag = "Data",
    responses(
        (status = 200, description = "Returns the retrieved key/value data.", body=Vec<Value>, example=json!([{"created_at": Utc::now().naive_utc(), "col1": 42, "col2": 51.234, "col3": "Hello"}])),
        (status = 401, description= "Returns an unauthorized error if access is not permitted."),
        (status = 500, description= "Returns an error if the sensor does not exist or the data couldn't be retrieved."),
    ),
)]

#[get("/data/sensor/{id}/load")]
async fn get_sensor_data_handler(path: web::Path<uuid::Uuid>, params: web::Query<DataLoadRequestParams>, state: web::Data<AppState>) -> impl Responder {
    let sensor_id = path.into_inner();

    // Retrieves the api key if it exists and is valid

    let api_key = match params.key {
        Some(key) => cache::request_api_key(key, &state).await,
        None => None
    };

    // Verifies key or guest access

    let has_access = match api_key {
        Some(key) => key.sensor_id == sensor_id && key.operation == DBOperation::READ,
        None => policy::require_sensor_permission(None, sensor_id, UserSensorPerm::Read, &state).await.is_none()
    };

    if !has_access {
        return policy::unauthorized("No permissions to read sensor data!".to_string()).unwrap();
    }

    // By default we enforce a limit of 10 rows if no limit is set
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
    use actix_http::Method;
    use actix_web::http::StatusCode;
    use async_std::task;
    use chrono::{NaiveDateTime, Utc};
    use serde_json::Map;
    use super::*;
    use sqlx::PgPool;
    use uuid::Uuid;
    use crate::database::models::db_structs::DBOrdering;
    use crate::test_utils::tests::{create_test_api_keys, create_test_app, create_test_sensors, execute_request, john};

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_ingest_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let target_sensor_own = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();
        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor2").unwrap();
        let target_sensor_not_allowed = test_sens.iter().find(|(name, _)| name == "MySensor4").unwrap();
        let public_sensor = test_sens.iter().find(|(name, _)| name == "MySensor5").unwrap();

        let payload = {
            json!({
                "col1": 42,
                "col2": 56.789,
                "col3": "Hello",
            })
        };

        // --- Ingest allowed (his own) sensor data as john with key - Should succeed ---

        let api_key_write = test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_own.1 && k.operation == DBOperation::WRITE).unwrap().id;

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest?key={}", target_sensor_own.1, api_key_write), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // --- Ingest allowed sensor data as john with key - Should succeed ---

        let api_key_write = test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1 && k.operation == DBOperation::WRITE).unwrap().id;

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest?key={}", target_sensor_allowed.1, api_key_write), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // --- Ingest allowed sensor as john with valid key but wrong op (READ) - Should fail ---

        let api_key_read = test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1 && k.operation == DBOperation::READ).unwrap().id;

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest?key={}", target_sensor_allowed.1, api_key_read), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::UNAUTHORIZED, &app).await;
        
        // --- Ingest un-allowed sensor with valid key (john) of wrong sensor - Should fail ---

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest?key={}", target_sensor_not_allowed.1, api_key_write), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Ingest sensor with invalid key - Should fail ---

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest?key={}", target_sensor_not_allowed.1, Uuid::new_v4()), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Ingest non-existing sensor without key - Should fail ---

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest", Uuid::new_v4()), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Ingest public sensor without key - Should succeed ---

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest", public_sensor.1), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // --- Ingest public sensor with valid key of wrong sensor - Should fail ---

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest?key={}", public_sensor.1, api_key_write), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Ingest public sensor with invalid key - Should succeed ---

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest?key={}", public_sensor.1, Uuid::new_v4()), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // --- Ingest various invalid data to public sensor - Should succeed ---

        fn compare_payload(payload: &Value, entry: &Map<String, Value>) -> bool {
            match (payload, entry) {
                (Value::Object(sub_map), super_map) => {
                    sub_map.iter().all(|(k, v)| super_map.get(k) == Some(v))
                }
                _ => false,
            }
        }

        let req_payload = DataLoadRequestParams {
            key: None,
            limit: Some(1),
            ordering: Some(DBOrdering::DESC),
            from: None,
            to: None,
        };

        // --- Some columns invalid - should be inserted as NULLs

        let payload = {
            json!({
                "col1": "42",
                "col2": "56.789",
                "col3": "42",
            })
        };

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest", public_sensor.1), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // Check result

        let body = execute_request(&format!("/api/data/sensor/{}/load", public_sensor.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

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

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest", public_sensor.1), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // Check result

        let body = execute_request(&format!("/api/data/sensor/{}/load", public_sensor.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

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

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest", public_sensor.1), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // Check result

        let body = execute_request(&format!("/api/data/sensor/{}/load", public_sensor.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

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

        let _ = execute_request(&format!("/api/data/sensor/{}/ingest", public_sensor.1), Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_get_data_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let target_sensor_own = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();
        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor2").unwrap();
        let target_sensor_not_allowed = test_sens.iter().find(|(name, _)| name == "MySensor4").unwrap();
        let public_sensor = test_sens.iter().find(|(name, _)| name == "MySensor5").unwrap();

        let mut req_payload = DataLoadRequestParams::default();

        fn compare_payload(payload: &Value, entry: &Map<String, Value>) -> bool {
            match (payload, entry) {
                (Value::Object(sub_map), super_map) => {
                    sub_map.iter().all(|(k, v)| super_map.get(k) == Some(v))
                }
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
                "col2": 56.789,
                "col3": "Hallo",
            })};

            payloads.push(payload);
        }

        let time_start: NaiveDateTime = Utc::now().naive_utc();
        let mut time_between: NaiveDateTime = Default::default();
        
        let api_key_write = test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_own.1 && k.operation == DBOperation::WRITE).unwrap().id;

        for (index, payload) in payloads.iter().enumerate() {
            execute_request(&format!("/api/data/sensor/{}/ingest", target_sensor_own.1), Method::POST, Some(vec![("key".to_string(),api_key_write.to_string())]),
                            Some(payload.clone()), None,
                            StatusCode::OK, &app).await;

            task::sleep(core::time::Duration::from_millis(25)).await; // A little bit of time for time tracking
            
            if index == 4 {
                time_between = Utc::now().naive_utc();
            }
        }

        let time_end: NaiveDateTime = Utc::now().naive_utc();

        // --- Get own sensor data with valid key (john) - Should succeed ---

        let api_key_read = test_keys.iter().find(|key| key.sensor_id == target_sensor_own.1 && key.user_id == john().id && key.operation == DBOperation::READ).unwrap().id;
        req_payload.key = Some(api_key_read);

        let body = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_own.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), payloads.len()); // We retrieved all data

        // Check if data is correct

        for (index, elm) in elms.iter().enumerate() {
            assert!(compare_payload(payloads.get(index).unwrap(), elm.as_object().unwrap()));
        }

        // --- Perform various allowed data retrievals to check correctness of data ---

        // Get only 5 elements
        let req_payload = DataLoadRequestParams {
            key: Some(api_key_read),
            limit: Some(5),
            ordering: None,
            from: None,
            to: None,
        };

        let body = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_own.1), Method::GET, Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::OK, &app).await;

        assert_eq!(body.as_array().unwrap().len(), 5);

        // Get the first (oldest) element
        let req_payload = DataLoadRequestParams {
            key: Some(api_key_read),
            limit: Some(1),
            ordering: Some(DBOrdering::ASC),
            from: None,
            to: None,
        };

        let body = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_own.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

        let elm = body.as_array().unwrap().first().unwrap();
        assert!(compare_payload(payloads.first().unwrap(), elm.as_object().unwrap()));

        // Get the last (newest) element
        let req_payload = DataLoadRequestParams {
            key: Some(api_key_read),
            limit: Some(1),
            ordering: Some(DBOrdering::DESC),
            from: None,
            to: None,
        };

        let body = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_own.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

        let elm = body.as_array().unwrap().first().unwrap();
        assert!(compare_payload(payloads.last().unwrap(), elm.as_object().unwrap()));

        // Get the first 5 elements based on time
        let req_payload = DataLoadRequestParams {
            key: Some(api_key_read),
            limit: None,
            ordering: None,
            from: Some(time_start),
            to: Some(time_between),
        };

        let body = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_own.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), 5);

        for (index, elm) in elms.iter().enumerate() {
            assert!(compare_payload(payloads.get(index).unwrap(), elm.as_object().unwrap()));
        }

        // Get the last 5 elements based on time
        let req_payload = DataLoadRequestParams {
            key: Some(api_key_read),
            limit: None,
            ordering: None,
            from: Some(time_between),
            to: Some(time_end),
        };

        let body = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_own.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), 5);
        
        for (index, elm) in elms.iter().enumerate() { // 5 last elements are ASC sorted
            assert!(compare_payload(payloads.get(5 + index).unwrap(), elm.as_object().unwrap()));
        }

        // All together, last 3 elements (of 5) desc
        let mut req_payload = DataLoadRequestParams {
            key: Some(api_key_read),
            limit: Some(3),
            ordering: Some(DBOrdering::DESC),
            from: Some(time_between),
            to: Some(time_end),
        };

        let body = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_own.1), Method::GET, Some(req_payload.to_vector()),
                                    None::<Value>, None,
                                    StatusCode::OK, &app).await;

        let elms = body.as_array().unwrap();

        assert_eq!(elms.len(), 3);

        for (index, elm) in elms.iter().enumerate() {
            assert!(compare_payload(payloads.get(9 - index).unwrap(), elm.as_object().unwrap()));
        }

        // --- Get allowed sensor data with valid key (john) - Should succeed ---

        req_payload.key = Some(test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1 && k.operation == DBOperation::READ).unwrap().id);

        let _ = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_allowed.1), Method::GET, Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::OK, &app).await;

        // --- Get not allowed sensor data with valid key (john) of wrong sensor - Should fail ---

        let _ = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_not_allowed.1), Method::GET,Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Get allowed sensor data with valid key (john) but wrong op (write) - Should fail ---

        req_payload.key = Some(test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1 && k.operation == DBOperation::WRITE).unwrap().id);

        let _ = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_allowed.1), Method::GET,Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Get non-existing sensor data without key - Should fail ---

        req_payload.key = None;
        let _ = execute_request(&format!("/api/data/sensor/{}/load", Uuid::new_v4()), Method::GET,Some(DataLoadRequestParams::default().to_vector()),
                                None::<Value>, None,
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Get non-public sensor data without key - Should fail ---

        let _ = execute_request(&format!("/api/data/sensor/{}/load", target_sensor_not_allowed.1), Method::GET,Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Get public sensor data without key - Should succeed ---

        let _ = execute_request(&format!("/api/data/sensor/{}/load", public_sensor.1), Method::GET,Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::OK, &app).await;

        // --- Get public sensor data with valid key of wrong sensor - Should fail ---

        req_payload.key = Some(test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1 && k.operation == DBOperation::WRITE).unwrap().id);

        let _ = execute_request(&format!("/api/data/sensor/{}/load", public_sensor.1), Method::GET,Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Get public sensor data with invalid key - Should succeed ---

        req_payload.key = Some(Uuid::new_v4());
        let _ = execute_request(&format!("/api/data/sensor/{}/load", public_sensor.1), Method::GET,Some(req_payload.to_vector()),
                                None::<Value>, None,
                                StatusCode::OK, &app).await;
    }
}