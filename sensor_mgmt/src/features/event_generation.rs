use crate::{
    database::models::events::{EventEngineState, LogEvent, LOG_EVENTS_GENERAL_CHANNEL},
    handler::models::{requests::TransportProto, telelmetry::OTelData},
    state::AppState,
};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    web::Data,
    Error,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    time::Instant,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tracing::{debug, error, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

// Event generation middleware
pub struct EventGenerator;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for EventGenerator
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = GenerateLogEventMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(GenerateLogEventMiddleware { service }))
    }
}

pub struct GenerateLogEventMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for GenerateLogEventMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();
        let start = Instant::now();

        let state = req.app_data::<Data<AppState>>().unwrap().clone();

        let otel = OTelData::generate();

        // Call the handler
        let fut = self.service.call(req);
        Box::pin(
            async move {
                let res = fut.await?;

                // Record the result of the request
                match state.events.clone().unwrap().les_chan.send(LogEvent::new(
                    otel,
                    start.elapsed(),
                    TransportProto::HTTP,
                    path,
                    res.status(),
                )) {
                    Ok(_) => (),
                    Err(err) => error!("failed to send LogEvent with {}", err),
                }
                // TODO handle err result
                Ok(res)
            }, //.instrument(span),
        )
    }
}

/// -----------------------------------
/// this recieves events from API handlers and inserts them into their persistence domain
///
/// This function starts the Logging & Events Service that send LogEvents to the DB.
///
/// Returns a channel where to send the LogEvents to.
pub fn init_event_service(state: AppState) -> EventEngineState {
    // Create a channel to which all log events will be sent
    // TODO find a good minimal buffer size?
    let (tx, rx) = mpsc::unbounded_channel::<LogEvent>();

    // Spawn a task that recieves log events and publishes them via the DB
    tokio::task::spawn(log_event_db_relay(state.clone(), rx));

    // Return the the channel where the log wrapper should send the messages to
    EventEngineState { les_chan: tx }
}

async fn log_event_db_relay(state: AppState, mut rx: UnboundedReceiver<LogEvent>) {
    if rx.is_closed() {
        panic!("log_event_db_relay called with closed rx!?");
    }

    info!("[LES] Waiting for incoming log events");
    loop {
        // Listen for incoming log events and relay them via the DB
        tokio::select! {
            // TODO it is possible to send multiple events at once so maybe we could create a buffer for incoming
            // events later on?
            opt_evt = rx.recv() => {
                match opt_evt {
                    Some(evt) => {

                        // Set OTel context
                        let parent_otel_ctx = evt.otel.context.extract();
                        let span = tracing::info_span!(
                            "process_log_event",
                            event.channel = LOG_EVENTS_GENERAL_CHANNEL,
                            otel.kind = "CONSUMER",
                        );
                        span.set_parent(parent_otel_ctx);
                        let _guard = span.enter();

                        let mut tx = state.db.begin().await.unwrap();

                        // Check if current filter config drops the event
                        if !default_filter(&evt){
                            debug!("[LES] {:?} dropped due to current filter config", evt);
                            continue;
                        }

                        // Extract senor ID if it exists
                        let sensor_id = extract_sensor_uuid(&evt);

                        // store the event
                        match sqlx::query("INSERT INTO log_events(t, sensor_id, data) VALUES($1, $2, $3)")
                            .bind(evt.t)
                            .bind(sensor_id)
                            .bind(serde_json::to_value(&evt).unwrap() as serde_json::Value)
                            .execute(&mut *tx)
                            .await {
                            Ok(_) => (),
                            Err(err) =>{error!("[LES] event insertion failed with: {}", err); continue;},
                        }

                        //if a sensor ID exists then publish on that channel instead of the general channel
                        let channel = match sensor_id {
                            Some(id) => format!("sensor/{}", id),
                            None => LOG_EVENTS_GENERAL_CHANNEL.to_string(),
                        };
                        // notify listeners
                        match sqlx::query(
r#"select pg_notify(chan, payload)
from (
        values ($1, $2)
    ) notifies(chan, payload)"#,
                        )
                        .bind(&channel)
                        .bind(serde_json::to_string(&evt).unwrap())
                        .execute(&mut *tx)
                        .await {
                            Ok(_) => (),
                            Err(err) => error!("[LES] event insertion failed with: {}", err),
                        }

                        match tx.commit().await {
                            Ok(_) => (),
                            Err(err) => error!("[LES] Commit failed with: {}", err),
                        }

                        if evt.status == 500 {
                            error!("[LES] {:?} on {}", evt, channel);
                        }
                        else if evt.status >= 400{
                            warn!("[LES] {:?} on {}", evt, channel);
                        } else {
                            info!("[LES] {:?} on {}", evt, channel);
                        }
                    },
                    None => {
                        info!("[LES] Channel closed");
                        break;
                    }
                }
            }
        }
    }
}

// Extract the uuid of a sensor
fn extract_sensor_uuid(e: &LogEvent) -> Option<Uuid> {
    return match e.path.split_once("/api/sensors/") {
        Some(p) => match p.1.split_once("/") {
            Some(p2) => match Uuid::parse_str(p2.0) {
                Ok(id) => Some(id),
                Err(err) => {
                    error!("{}", err);
                    None
                }
            },
            // We had the prefix so we should have an uuid
            None => match Uuid::parse_str(p.1) {
                Ok(id) => Some(id),
                // some api calls like list and create dont have a uuid but the correct prefix
                Err(_) => None,
            },
        },
        None => None,
    };
}

fn default_filter(e: &LogEvent) -> bool {
    // We only care about sensor data manipulation events
    return is_sensor_data_manipulation_path(&e.path);
}

// Define the known parts of the path
const PREFIX: &str = "/api/sensors/";
const UUID_LEN: usize = 36; // A UUID string is always 36 characters long

// Define the specific suffixes we care about
const INGEST_SUFFIX: &str = "/data/ingest";
const DELETE_SUFFIX: &str = "/data/delete";

// MQTT (w/o api_key)
const MQTT_PATH_LEN: usize = PREFIX.len() + UUID_LEN;
const MQTT_PATH_TRALILING_SLASH_LEN: usize = PREFIX.len() + UUID_LEN + 1;
const MQTT_PATH_WITH_KEY_LEN: usize = PREFIX.len() + UUID_LEN + 1 + UUID_LEN;

// HTTP all interesting cases have the same length
const HTTP_PATH_LEN: usize = PREFIX.len() + UUID_LEN + INGEST_SUFFIX.len();

/// This function returns true if the given path is a data manipulation path
/// In others cases false is returned
fn is_sensor_data_manipulation_path(path: &str) -> bool {
    let correct_size = match path.len() {
        // MQTT
        MQTT_PATH_LEN => true,
        MQTT_PATH_TRALILING_SLASH_LEN => true,
        MQTT_PATH_WITH_KEY_LEN => true,
        // HTTP
        HTTP_PATH_LEN => true,
        _ => false,
    };
    if !correct_size {
        return false;
    }

    // 2. Prefix check
    if !path.starts_with(PREFIX) {
        return false;
    }

    // 3. Specific suffix check:
    // Calculate the start index for the suffix segment (after prefix and UUID)
    let suffix_segment_start_index = PREFIX.len() + UUID_LEN;
    // Ensure the remainder of the string (from suffix_segment_start_index onwards)
    // starts with either INGEST_SUFFIX or DELETE_SUFFIX.
    if let Some(remainder) = path.get(suffix_segment_start_index..) {
        return remainder.starts_with(INGEST_SUFFIX)
            || remainder.starts_with(DELETE_SUFFIX)
            || remainder.eq("/")
            || remainder.len() == 1 + UUID_LEN
            || remainder.is_empty();
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_UUID: &str = "d0c7b5b6-4ece-4ab2-b1c8-791afd8e5b3e";

    #[test]
    fn test_valid_ingest_path() {
        let path = format!("{}{}{}", PREFIX, EXAMPLE_UUID, INGEST_SUFFIX);
        assert!(is_sensor_data_manipulation_path(&path));
    }

    #[test]
    fn test_valid_delete_path() {
        let path = format!("{}{}{}", PREFIX, EXAMPLE_UUID, DELETE_SUFFIX);
        assert!(is_sensor_data_manipulation_path(&path));
    }

    #[test]
    fn test_valid_mqtt() {
        let path = format!("{}{}", PREFIX, EXAMPLE_UUID);
        assert!(is_sensor_data_manipulation_path(&path));

        let path = format!("{}{}/", PREFIX, EXAMPLE_UUID);
        assert!(is_sensor_data_manipulation_path(&path));

        let path = format!("{}{}/{}", PREFIX, EXAMPLE_UUID, EXAMPLE_UUID);
        assert!(is_sensor_data_manipulation_path(&path));
    }

    #[test]
    fn test_too_short_overall() {
        assert!(!is_sensor_data_manipulation_path(
            "/api/sensors/uuid/data/ingest"
        )); // UUID too short
        assert!(!is_sensor_data_manipulation_path(
            "/api/sensors/d0c7b5b6-4ece-4ab2-b1c8-791afd8e5b3/data/ingest"
        )); // UUID 1 char too short
    }

    #[test]
    fn test_invalid_prefix() {
        let path = format!("/sensors/{}{}", EXAMPLE_UUID, INGEST_SUFFIX);
        assert!(!is_sensor_data_manipulation_path(&path));
    }

    #[test]
    fn test_invalid_suffix_segment() {
        let path = format!("{}{}/data/load", PREFIX, EXAMPLE_UUID); // Not ingest or delete
        assert!(!is_sensor_data_manipulation_path(&path));

        let path = format!("{}{}/data/", PREFIX, EXAMPLE_UUID); // Missing ingest/delete
        assert!(!is_sensor_data_manipulation_path(&path));
    }

    #[test]
    fn test_path_with_extra_elements_after_valid_suffix() {
        let uuid = "";
        let path = format!("{}{}{}/extra_data", PREFIX, uuid, INGEST_SUFFIX);
        // If the path *must* end exactly with /ingest or /delete, this should be false.
        // As per the prompt "All others are not interesting i think", it implies these are the full, exact patterns.
        assert!(!is_sensor_data_manipulation_path(&path));
    }

    #[test]
    fn test_empty_string() {
        assert!(!is_sensor_data_manipulation_path(""));
    }

    #[test]
    fn test_just_prefix() {
        assert!(!is_sensor_data_manipulation_path("/api/sensors/"));
    }
}
