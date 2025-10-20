use crate::database::data_db::add_sensor_data;
use crate::database::models::db_structs::DBOperation;
use crate::features::user_sens_perm::UserSensorPerm;
use crate::features::{cache, sensor_data_transform};
use crate::handler::policy;
use crate::state::AppState;
use crate::utils::AppError;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/*

Hierachical Sensor Monitoring
Only available during runtime thus not saved in DB.

Base Stats per Sensor
{
    first msg: Time

    last success ingest: Time

    Ringbuffer: Vec<Error> {msg: String, occured: Time, inputData: Byte}
    -> plus Transport level msgs
}
-> Extra stats per protocol
{
    Per sensor {
        recv,
        err,
        err_auth,
        succ,
    }
}
*/

pub type RuntimeIngestStats = Arc<RwLock<IngestStats>>;

#[derive(Default, Clone)]
pub struct IngestStats {
    // Once the system has been started this represents the first time an ingest has been recieved
    // Useful to calculate message rate during runtime
    first_msg: Option<Instant>,

    // The last time a successfull ingest has happened
    last_success_ingest: Option<Instant>,
    // When errors occure we store some data to help figure out why the ingest has failed
    //last_errors: Vec<IngestStatsErrors>,
}

pub struct IngestStatsErrors {
    t: Instant,
    msg: String,
    input: bytes::Bytes,
}

impl IngestStats {
    pub fn new() -> RuntimeIngestStats {
        Arc::new(RwLock::new(IngestStats::default()))
    }
}

/* ------------------------------------------------ API ------------------------------------------------------------ */

/// Insert data into the db for sensor_id using api_key for access control.
/// The returned boolean value indicates wether an entry has been produced
pub async fn ingest_data_buisness_logic(
    sensor_id: uuid::Uuid,
    api_key: Option<uuid::Uuid>,
    data: bytes::Bytes,
    state: &AppState,
) -> anyhow::Result<bool, AppError> {
    // TODO Set first msg if its not already set

    // Retrieve key and check access
    let api_key = match api_key {
        Some(key) => cache::request_api_key(key, &state).await,
        None => None,
    };
    let has_access = match api_key {
        Some(key) => key.sensor_id == sensor_id && key.operation == DBOperation::WRITE,
        None => policy::require_sensor_permission(None, sensor_id, UserSensorPerm::Write, &state)
            .await
            .is_none(),
    };
    if !has_access {
        return Err(AppError::unauthorized_generic2()).into();
    }

    // Data sanity check
    if data.len() == 0 {
        return AppError::internal("missing data to insert".to_string());
    }

    // Get all required info on the sensor
    let sensor_opt = cache::request_sensor(sensor_id, &state).await;
    if sensor_opt.is_none() {
        return AppError::internal(format!("could not find sensor with id: '{}'", sensor_id));
    }
    let sensor = Arc::new(sensor_opt.unwrap());

    // Transform data into ingestable format
    let tr_res = sensor_data_transform::transform(sensor.clone(), data, &state).await;
    if let Err(err) = tr_res {
        return AppError::internal(format!("data transform failed with: {}", err));
    }
    let data = tr_res.unwrap();

    // If the vec is empty we dont need to bother with query creation
    if data.len() == 0 {
        return Ok(false);
    }

    // insert data into db
    let res = add_sensor_data(sensor, &data, state.clone()).await;
    if res.is_err() {
        return AppError::db(format!("{:?}", res));
    }

    Ok(true)
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::database::models::db_structs::DBOrdering;
    use crate::handler::data_ingest::mqtt::tests::mqtt_client_publish;
    use crate::handler::models::requests::{
        DataLoadRequestParams, SensorDataIngestEntry, TransportProto,
    };
    use crate::test_utils::tests::{
        create_test_api_keys, create_test_app, create_test_sensors, execute_request, john,
    };
    use actix_http::{Method, Request};
    use actix_web::body::BoxBody;
    use actix_web::dev::{Service, ServiceResponse};
    use actix_web::http::StatusCode;
    use async_std::task;
    use chrono::{NaiveDateTime, Utc};
    use serde::Serialize;
    use serde_json::Map;
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn ingest_data<T>(
        sensor_id: uuid::Uuid,
        api_key: Option<uuid::Uuid>,
        proto: TransportProto,
        method: Method,
        params: Option<Vec<(String, String)>>,
        payload: Option<T>,
        token: Option<String>,
        expected_status: StatusCode,
        app: impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
        state: AppState,
    ) where
        T: Serialize + Clone,
    {
        match proto {
            TransportProto::HTTP => {
                let url = match api_key {
                    Some(key) => &format!("/api/sensors/{}/data/ingest?key={}", sensor_id, key),
                    None => &format!("/api/sensors/{}/data/ingest", sensor_id),
                };

                // panics on fail
                let _ =
                    execute_request(&url, method, params, payload, token, expected_status, &app)
                        .await;

                return;
            }
            TransportProto::MQTT => {
                let _ =
                    mqtt_client_publish(sensor_id, api_key, payload, expected_status, state).await;
            }
        }
    }

    fn compare_payload(payload: &Value, entry: &Map<String, Value>) -> bool {
        match (payload, entry) {
            (Value::Object(sub_map), super_map) => {
                sub_map.iter().all(|(k, v)| super_map.get(k) == Some(v))
            }
            _ => false,
        }
    }

    #[sqlx::test(
        migrations = "../migrations",
        fixtures(
            "../fixtures/users.sql",
            "../fixtures/roles.sql",
            "../fixtures/user_roles.sql"
        )
    )]
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

        let api_key_write_own = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_own.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        let api_key_read = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_allowed.1
                    && k.operation == DBOperation::READ
            })
            .unwrap()
            .id;

        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_allowed.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        let payload = json!({
            "col1": 42,
            "col2": 56.789,
            "col3": "Hello",
        });
        let data_entry = SensorDataIngestEntry::from_json(payload, None);

        for proto in TransportProto::iterator() {
            // --- Ingest allowed (his own) sensor data as john with key - Should succeed ---

            ingest_data(
                target_sensor_own.1,
                Some(api_key_write_own),
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest allowed sensor data as john with key - Should succeed ---

            ingest_data(
                target_sensor_allowed.1,
                Some(api_key_write),
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest allowed sensor as john with valid key but wrong op (READ) - Should fail ---

            ingest_data(
                target_sensor_allowed.1,
                Some(api_key_read),
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::UNAUTHORIZED,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest un-allowed sensor with valid key (john) of wrong sensor - Should fail ---

            ingest_data(
                target_sensor_not_allowed.1,
                Some(api_key_write),
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::UNAUTHORIZED,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest sensor with invalid key - Should fail ---

            ingest_data(
                target_sensor_not_allowed.1,
                Some(Uuid::new_v4()),
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::UNAUTHORIZED,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest non-existing sensor without key - Should fail ---

            ingest_data(
                Uuid::new_v4(),
                None,
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::INTERNAL_SERVER_ERROR,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest public sensor without key - Should succeed ---

            ingest_data(
                public_sensor.1,
                None,
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest public sensor with valid key of wrong sensor - Should fail ---

            ingest_data(
                public_sensor.1,
                Some(api_key_write),
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::UNAUTHORIZED,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest public sensor with invalid key - Should succeed ---

            ingest_data(
                public_sensor.1,
                Some(Uuid::new_v4()),
                *proto,
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
            )
            .await;

            // --- Ingest various invalid data to public sensor - Should succeed ---

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

            ingest_data(
                public_sensor.1,
                None,
                *proto,
                Method::POST,
                None,
                Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
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

            ingest_data(
                public_sensor.1,
                None,
                *proto,
                Method::POST,
                None,
                Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
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

            ingest_data(
                public_sensor.1,
                None,
                *proto,
                Method::POST,
                None,
                Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
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

            ingest_data(
                public_sensor.1,
                None,
                *proto,
                Method::POST,
                None,
                Some(vec![SensorDataIngestEntry::from_json(payload, None)]),
                None,
                StatusCode::INTERNAL_SERVER_ERROR,
                &app,
                state.clone(),
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

            ingest_data(
                public_sensor.1,
                None,
                *proto,
                Method::POST,
                None,
                Some(data_entries),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
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
                let time: NaiveDateTime = Utc::now().naive_utc()
                    + chrono::Duration::milliseconds((i * delay_per_tup) as i64);

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

            ingest_data(
                public_sensor.1,
                None,
                *proto,
                Method::POST,
                None,
                Some(data_entries),
                None,
                StatusCode::OK,
                &app,
                state.clone(),
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
    }
}
