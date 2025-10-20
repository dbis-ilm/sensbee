use crate::database::data_chain_db::load_inbound;
use crate::database::data_transformer_db::{self};
use crate::features::config::as_compose_service;
use crate::{
    database::models::sensor::FullSensorInfo, handler::models::requests::SensorDataIngestEntry,
    state::AppState,
};
use anyhow::anyhow;
use futures_util::{sink::SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::RwLock;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio::{net::TcpStream, sync::oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

/* ------------------------------------------------ Public API ------------------------------------------------------------ */

/// Takes input JSON data and transforms it into a specific output
/// Request flow
/// Send Transform Request
///     -> Transform Service
///     (<- Request script content)
///     (-> Send script content)
///     <- Transform result     
/// Return transform result
/// NOTE TODO this does not support transactions
pub async fn transform(
    sensor: Arc<FullSensorInfo>,
    data: bytes::Bytes,
    state: &AppState,
) -> anyhow::Result<Vec<SensorDataIngestEntry>> {
    match load_inbound(sensor.id, &state.db).await? {
        Some(transformer_id) => {
            debug!("using transformer before ingest");
            Span::current().context().with_value(transformer_id);

            let res = get_transformed_data(
                &transformer_id,
                String::from_utf8(data.to_vec())?,
                &state.data_transform,
            )
            .await?;

            return Ok(serde_json::from_slice::<Vec<SensorDataIngestEntry>>(&res)?);
        }
        None => {
            debug!("No inbound transformer found");
            Ok(serde_json::from_slice::<Vec<SensorDataIngestEntry>>(&data)?)
        }
    }
}

/// This is the internal entrypoint for script execution
pub async fn get_transformed_data(
    id: &Uuid,
    data: String,
    ts: &TransformService,
) -> anyhow::Result<bytes::Bytes> {
    // TODO an error here should also generate an event?
    // path something with transform service and the id of the script
    // proto empty?
    // status 500?
    // msg: error message?

    // Should call the transform service now
    let (responder_tx, responder_rx) = oneshot::channel::<TransformServiceResponse>();

    ts.tx
        .send(TransformServiceRequest {
            script_id: *id,
            data: data,
            responder: responder_tx,
        })
        .await?;

    let res = responder_rx.await??;

    #[cfg(test)]
    debug!("transform result: '{:?}'", res);

    Ok(res)
}

/* ------------------------------------------------ Worker ------------------------------------------------------------ */

/// Internal entrypoint for the Transform Service API
///
/// Holds all members that are intended to be used by outward facing functions.
pub struct TransformService {
    // channel to send DataTransform jobs to the task
    tx: mpsc::Sender<TransformServiceRequest>,

    // Stats for this service that may be viewed by external tools
    stats: Stats,
}

// ----
// Internal channel communication

type TransformServiceResponse = anyhow::Result<bytes::Bytes>;

/// Interal struct to be used when a request for the websocket task should send a request to the transform service.
/// Includes a responder oneshot channel where the result will be sent to.
pub struct TransformServiceRequest {
    pub(crate) script_id: Uuid,
    pub(crate) data: String,
    // Channel where the response should be send to
    pub(crate) responder: oneshot::Sender<TransformServiceResponse>,
}

// ----
// Internal stats for this service

#[derive(Default)]
pub struct TransformServiceStats {
    connected: bool,
    transform_errors: u64,
    transform_successs: u64,
    req_recv: u64,
}

#[derive(Clone)]
struct Stats(Arc<RwLock<TransformServiceStats>>);

impl Stats {
    pub fn new() -> Self {
        Stats(Arc::new(RwLock::new(TransformServiceStats::default())))
    }

    pub fn set_connected(&self, state: bool) {
        let mut s = self.0.write().unwrap();

        s.connected = state;
    }
    pub fn incr_err(&self) {
        let mut s = self.0.write().unwrap();

        s.transform_errors += 1;
    }
    pub fn incr_succ(&self) {
        let mut s = self.0.write().unwrap();

        s.transform_successs += 1;
    }
    pub fn incr_req(&self) {
        let mut s = self.0.write().unwrap();

        s.req_recv += 1;
    }
}

// ----
// External

/// The type of request to be either send or recieved.
/// The transform services expects this to be a number!
#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
enum ReqType {
    Unknown = 0,
    Error = 1,
    Request = 2,
    GetScript = 3,
    SendScript = 4,
}

/// Represents a package sent to or recieved from the transform service on the WebSocket connection.
#[derive(Debug, Deserialize, Serialize)]
struct TSRequestBase {
    // The uuid of the script that shall be used
    script_id: uuid::Uuid,
    // The type of this message
    #[serde(rename = "type")]
    req_type: ReqType,
    // Additional data depending on the type of the message
    data: String,
}

/// The main task that manages the WebSocket connection.
/// If the connection is lost it will try to reestablish.
async fn websocket_task(
    db: PgPool,
    mut receiver: mpsc::Receiver<TransformServiceRequest>,
    stats: Stats,
) {
    //#[cfg(test)]
    //let url = "ws://localhost:9002";
    //#[cfg(not(test))]
    let url = format!("ws://{}:9002", as_compose_service("sb-service-transform"));

    let mut ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>> = None;

    // NOTE there is propably something better suited for the task
    // Used to store the request_id <-> responder channel relation
    let mut responder_map: HashMap<uuid::Uuid, oneshot::Sender<TransformServiceResponse>> =
        HashMap::new();

    let mut reconnect_delay = 1;

    info!("[DTS] connecting...");

    loop {
        // Attempt to connect if not connected
        if ws_stream.is_none() {
            stats.set_connected(false);

            match connect_async(url.clone()).await {
                Ok((stream, _)) => {
                    ws_stream = Some(stream);

                    reconnect_delay = 1;

                    stats.set_connected(true);

                    info!("[DTS] connected");
                }
                Err(err) => {
                    error!("[DTS] Connection to '{url}' failed with {err}, retrying in {reconnect_delay} seconds...");

                    tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                    // increase delay when errors are consecutive
                    reconnect_delay = reconnect_delay * 2;
                    if reconnect_delay > 30 {
                        reconnect_delay = 30;
                    }

                    continue;
                }
            }
        }

        /*
           Concept:

           If a sensor ingest data that needs to be transformed to conform to the sensor table schema it is sent via a channel to this task.
           This task relays that transformation request to an external transformer service.

           Allowed internal requests:
               SensBee:
                   data-transform
               Transform-service:
                   get-transform

           Responses:
               SensBee:
                   Transformed Data || Error
               Transform-service:
                   Transform Script || Error

        */

        if let Some(stream) = &mut ws_stream {
            tokio::select! {
                // Prioritize incoming messages sent by internal tasks to the transform service
                Some(msg) = receiver.recv() => {

                    debug!("[DTS] recieved TransformServiceRequest");

                    // Create a transform service request
                    let tsr = serde_json::to_string(&TSRequestBase{
                        req_type: ReqType::Request,
                        script_id: msg.script_id,
                        data: msg.data,
                    });
                    if let Err(err) = tsr {
                        let resp_send = msg.responder.send(Err(anyhow!("serde_json::to_string failed with : {}", err)));
                        if let Err(err) = resp_send {
                            error!("[DTS] Failed to send msg to responder with: {:?}", err);
                            continue;
                        }
                        break;
                    }

                    // Convert to external service request and send it
                    let s = stream.send(tsr.unwrap().into()).await;
                    if let Err(err) = s {
                        let _ = msg.responder.send(Err(anyhow!("stream.send failed with: {}", err)));
                        break;
                    }

                    // Store responder channel for newly created request
                    responder_map.insert(msg.script_id, msg.responder);

                    debug!("[DTS] TransformServiceRequest {} sent to transform_service", msg.script_id);

                    stats.incr_req();
                },
                // Handle incoming messages from the external transform service
                Some(service_ws_msg) = stream.next() => {
                    match service_ws_msg {
                        // All incoming message should be utf8 text
                        Ok(Message::Text(msg)) => {
                            debug!("[DTS] received message: {}", msg);

                            // Parse the incoming message into a common format
                            let m  = serde_json::from_str::<TSRequestBase>(&msg.to_string());
                            if let Err(err) = &m {
                                error!("[DTS] failed to parse incoming ws message: {:?}", err);
                            }
                            let req = m.unwrap();

                            match req.req_type {
                                // Any error that has happened in the transform service
                                ReqType::Error =>  {
                                    match responder_map.remove(&req.script_id) {
                                        None => {
                                            error!("[DTS] recieved an error with an invalid req.id: {:?}", req);
                                        },
                                        Some(resp) => {
                                            let res = resp.send(Err(anyhow!("transform service error: '{}'", req.data)));
                                            if let Err(err) = res {
                                                error!("[DTS] get_data_transform_script: {:?}", err);
                                            }
                                        },
                                    }
                                },
                                ReqType::Request => {
                                    // A transform request has been fullfilled
                                    match responder_map.remove(&req.script_id) {
                                        None => {
                                            error!("[DTS] recieved a request with an invalid req.id: {:?}", req);
                                        },
                                        Some(resp) => {
                                            let res = resp.send(Ok(req.data.into()));
                                            if let Err(err) = res {
                                                error!("[DTS] resp.send failed with: {:?}", err);
                                            }
                                        },
                                    }
                                },
                                ReqType::GetScript => {

                                    let script_id =  req.script_id;

                                    // The data transform script is not present in the service cache so we need to retrieve it and send it back
                                    let resp = match data_transformer_db::load(script_id, &db.clone()).await {
                                        Ok(s) => s,
                                        Err(err)=>{
                                            error!("[DTS] get_data_transform_script: {:?}", err);
                                            break;
                                        }
                                    };

                                    let s = serde_json::to_string(&TSRequestBase{
                                        script_id: req.script_id,
                                        req_type: ReqType::SendScript,
                                        data: resp.script,
                                    });
                                    if let Err(err) = s {
                                        error!("[DTS] serde_json::to_string: {:?}", err);
                                        break;
                                    }

                                    match stream.send(s.unwrap().into()).await {
                                        Ok(_) => {},
                                        Err(err) => {
                                            error!("[DTS] stream.feed: {:?}", err);
                                            break;
                                        },
                                    }
                                }
                                // These should not be recieved!
                                ReqType::Unknown =>  error!("[DTS] A ReqType::Unknown has been recieved"),
                                ReqType::SendScript => error!("[DTS] A ReqType::SendScript has been recieved"),
                            }

                            stats.incr_succ();
                        },
                        Ok(Message::Close(_)) => {

                            info!("[DTS] Received close frame");

                            ws_stream = None;
                        },
                        Ok(msg) => {
                            error!("[DTS] Received unhandeld message on ws: {}", msg);

                            stats.incr_err();
                        },
                        Err(err) => {
                            error!("[DTS] stream.next() error: {}", err);

                            // Assume connection is broken, this enforces a reconnect
                            ws_stream = None;

                            stats.incr_err();
                        },
                    };
                },
                // If the channel is closed, the task should probably end
                else => break,
            }
        } else {
            // If not connected, just wait for a message on the channel or a delay
            // We already have a sleep in the connection logic,
            // but adding a small sleep here prevents a tight loop
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    info!("[DTS] WebSocket task shutting down.");
}

/// Starts the WebSocket background task.
/// Returns a sender that can be used to send messages to the task.
pub fn start_websocket_task(pool: PgPool) -> TransformService {
    // Create a channel with a buffer of, say, 100 messages
    let (sender, receiver) = mpsc::channel::<TransformServiceRequest>(100);

    let s = Stats::new();

    // Spawn the background task
    tokio::spawn(websocket_task(pool, receiver, s.clone()));

    // Return the sender
    TransformService {
        tx: sender,
        stats: s.clone(),
    }
}

// IDEA
// We could also use PL/v8 to execute these tasks

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use actix_http::{Method, StatusCode};
    use serde_json::json;
    use sqlx::PgPool;
    use std::{str::FromStr, time::Instant};
    use uuid::Uuid;

    use crate::{
        database::models::db_structs::DBOperation,
        handler::models::requests::SensorDataIngestEntry,
        test_utils::tests::{
            create_test_api_keys, create_test_app, create_test_sensors, execute_request, john,
            login,
        },
    };

    pub fn working_transform_script() -> serde_json::Value {
        return json!({
        "name":"A working test script",
        "script":"
            let inputData = data[0];
            let col1 = parseInt(inputData.col1);
            let col2 = parseFloat(inputData.col2);

            return [{'col1': col1, 'col2':col2,'col3':inputData.col3}];
    "});
    }

    #[sqlx::test(
        migrations = "../migrations",
        fixtures(
            "../handler/fixtures/users.sql",
            "../handler/fixtures/roles.sql",
            "../handler/fixtures/user_roles.sql"
        )
    )]
    async fn test_data_ingest_transform_service(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;
        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let token = login(&john(), &state).await;

        let target_sensor_own = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor")
            .unwrap();
        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_own.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        // --- Ingest some data -- should work ---

        let data_entry = SensorDataIngestEntry::from_json(
            json!({
                "col1": 42,
                "col2": 56.789,
                "col3": "Hello",
            }),
            None,
        );
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

        // --- Set transform script -- should work ---
        // Step 1: Create data_transformer
        let p = working_transform_script();
        let dt_id_resp = execute_request(
            &format!("/api/data_transformer/create"),
            Method::POST,
            None,
            Some(p),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let dt_id = Uuid::from_str(dt_id_resp.get("uuid").unwrap().as_str().unwrap()).unwrap();
        // Step 2: set inbound chain
        let _ = execute_request(
            &format!("/api/sensors/{}/data_chain/set", target_sensor_own.1),
            Method::POST,
            None,
            Some(json!({"chain": {"inbound":dt_id}})),
            Some(token.clone()),
            StatusCode::NO_CONTENT,
            &app,
        )
        .await;

        // --- this ingest should now use the transform -- should work ---

        let data_entry = SensorDataIngestEntry::from_json(
            json!({
                "col1": "42",
                "col2": "56.789",
                "col3": "Hello",
            }),
            None,
        );
        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                target_sensor_own.1, api_key_write
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;

        // delete the script again -- should work

        // TODO

        // use the normal ingest again -- should work

        // TODO

        // use a incompatible ingest -- should fail

        // TODO
    }

    #[sqlx::test(
        migrations = "../migrations",
        fixtures(
            "../handler/fixtures/users.sql",
            "../handler/fixtures/roles.sql",
            "../handler/fixtures/user_roles.sql"
        )
    )]
    async fn test_data_ingest_transform_service_bench(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;
        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let token = login(&john(), &state).await;

        let target_sensor_own = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor")
            .unwrap();
        let api_key_write = test_keys
            .iter()
            .find(|k| {
                k.user_id == john().id
                    && k.sensor_id == target_sensor_own.1
                    && k.operation == DBOperation::WRITE
            })
            .unwrap()
            .id;

        // --- ingest some data which should work

        let data_entry = SensorDataIngestEntry::from_json(
            json!({
                "col1": 42,
                "col2": 56.789,
                "col3": "Hello",
            }),
            None,
        );
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

        // time it

        let runs = 1000;
        let mut sum: u128 = 0;
        for _ in 0..runs {
            let now = Instant::now();

            let _ = execute_request(
                &format!(
                    "/api/sensors/{}/data/ingest?key={}",
                    target_sensor_own.1, api_key_write
                ),
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                Some(token.clone()),
                StatusCode::OK,
                &app,
            )
            .await;

            sum += now.elapsed().as_micros();
        }
        println!("{} ingest calls in avg: {}us", runs, sum / runs);

        // --- Set transform script
        // Step 1: Create data_transformer
        let p = working_transform_script();
        let dt_id_resp = execute_request(
            &format!("/api/data_transformer/create"),
            Method::POST,
            None,
            Some(p),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let dt_id = Uuid::from_str(dt_id_resp.get("uuid").unwrap().as_str().unwrap()).unwrap();
        // Step 2: set inbound chain
        let _ = execute_request(
            &format!("/api/sensors/{}/data_chain/set", target_sensor_own.1),
            Method::POST,
            None,
            Some(json!({"chain": {"inbound":dt_id}})),
            Some(token.clone()),
            StatusCode::NO_CONTENT,
            &app,
        )
        .await;

        // this ingest should now use the transform

        let data_entry = SensorDataIngestEntry::from_json(
            json!({
                "col1": "42",
                "col2": "56.789",
                "col3": "Hello",
            }),
            None,
        );
        let _ = execute_request(
            &format!(
                "/api/sensors/{}/data/ingest?key={}",
                target_sensor_own.1, api_key_write
            ),
            Method::POST,
            None,
            Some(vec![data_entry.clone()]),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;

        // now we check the average runtime

        let data_entry = SensorDataIngestEntry::from_json(
            json!({
                "col1": "42",
                "col2": "56.789",
                "col3": "Hello",
            }),
            None,
        );
        let mut sum: u128 = 0;
        for _ in 0..runs {
            let now = Instant::now();

            let _ = execute_request(
                &format!(
                    "/api/sensors/{}/data/ingest?key={}",
                    target_sensor_own.1, api_key_write
                ),
                Method::POST,
                None,
                Some(vec![data_entry.clone()]),
                Some(token.clone()),
                StatusCode::OK,
                &app,
            )
            .await;

            sum += now.elapsed().as_micros();
        }
        println!("{} ingest + transform calls in avg: {}us", runs, sum / runs);
    }
}
