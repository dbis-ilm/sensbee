use crate::database::models::events::LogEvent;
use crate::features::config::as_compose_service;
use crate::handler::data_ingest::ingest::ingest_data_buisness_logic;
use crate::handler::models::requests::TransportProto;
use crate::handler::models::telelmetry::OTelData;
use crate::state::AppState;
use crate::utils::AppError;
use actix_http::StatusCode;
use actix_web::ResponseError;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, info_span};
use uuid::Uuid;

/*

MQTT

watchdog:
    gets resumed when the subscriber fails and restarts it

subscriber:
    wildcard subscriber to recieve all events

*/

/* ------------------------------------------------ API ------------------------------------------------------------ */

/// External entrypoint to interact with the service
///
/// Currently the service is intended to be run in the background.
/// Thus is offers only some runtime stats which can be read externally.
#[derive(Clone)]
pub struct MQTT {
    // Statistics for the service
    stats: Stats,
}

/// Starts a tokio task that runs the mqtt subscriber. If the subscriber fails the error is logged and the subscriber gets restarted.
pub fn mqtt_service_init(state: AppState) -> MQTT {
    // By default we use a WebSocket connection
    let t = rumqttc::Transport::Ws;

    // Create stats which will be shared
    let share = Stats::new();

    // Spawn the actual task
    let s = share.clone();
    tokio::spawn(async move {
        debug!("[MQTT] subscriber starting.");

        loop {
            match start_mqtt_subscriber(state.clone(), t.clone(), s.clone()).await {
                Err(error) => {
                    error!("[MQTT] subscriber failed with: '{error:?}'. Restarting...");
                }
                Ok(_) => break,
            };
        }

        info!("[MQTT] subscriber stopped.");
    });

    MQTT {
        stats: share.clone(),
    }
}

/* ------------------------------------------------ Service Stats ------------------------------------------------------------ */

#[derive(Debug, Clone, Default)]
pub struct MQTTServiceStats {
    // indicates active connection to the mosquitto broker
    connected: bool,
    // Counts overall recieved packets of type Incoming::Publish
    packets_recv: u64,
    // Counts how many times packet parsing has failed
    err_parse: u64,

    per_sensor: HashMap<uuid::Uuid, MQTTSensorIngestStats>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MQTTSensorIngestStats {
    // Counts overall recieved packets of type Incoming::Publish for this sensor
    packets_recv: u64,
    // Counts how many times packet ingest has failed for this sensor
    err_ingest: u64,
    err_ingest_auth: u64,
    // Counts how many packets have been recieved and inserted in the db successfully for this sensor
    db_insert_succ: u64,
}

impl MQTTSensorIngestStats {
    pub fn res_sum(self) -> u64 {
        self.err_ingest + self.err_ingest_auth + self.db_insert_succ
    }
}

#[derive(Clone)]
struct Stats(Arc<RwLock<MQTTServiceStats>>);

impl Stats {
    pub fn new() -> Self {
        Stats(Arc::new(RwLock::new(MQTTServiceStats::default())))
    }

    // Returns a clone of the current stats
    pub fn read_stats(self) -> MQTTServiceStats {
        self.0.read().unwrap().deref().clone()
    }
    // If the entry does not exist a default struct will be returned
    pub fn read_sensor_stats(self, sensor_id: &uuid::Uuid) -> MQTTSensorIngestStats {
        match self.0.read().unwrap().deref().per_sensor.get(sensor_id) {
            Some(s) => (*s).clone(),
            None => MQTTSensorIngestStats::default(),
        }
    }

    pub fn get_connected(&self) -> bool {
        self.0.read().unwrap().connected
    }

    pub fn set_connected(&self, state: bool) {
        let mut s = self.0.write().unwrap();

        s.connected = state;
    }

    pub fn incr_recv(&self) {
        let mut s = self.0.write().unwrap();

        s.packets_recv += 1;
    }

    pub fn incr_err_parse(&self) {
        let mut s = self.0.write().unwrap();

        s.err_parse += 1;
    }

    // Per sensor stats
    // NOTE we guarantee that this is always called the first time with any new sensor_id which
    // is why the later functions can safely call unwrap on the entry
    pub fn incr_sensor_recv(&self, sensor_id: uuid::Uuid) {
        let mut s = self.0.write().unwrap();

        s.packets_recv += 1;

        match s.per_sensor.get_mut(&sensor_id) {
            Some(e) => e.packets_recv += 1,
            None => {
                let mut e = MQTTSensorIngestStats::default();
                e.packets_recv = 1;
                s.per_sensor.insert(sensor_id, e);
            }
        }
    }
    pub fn incr_sensor_err_ingest(&self, sensor_id: uuid::Uuid) {
        let mut s = self.0.write().unwrap();

        s.per_sensor.get_mut(&sensor_id).unwrap().err_ingest += 1;
    }
    pub fn incr_sensor_err_ingest_auth(&self, sensor_id: uuid::Uuid) {
        let mut s = self.0.write().unwrap();

        s.per_sensor.get_mut(&sensor_id).unwrap().err_ingest_auth += 1;
    }
    pub fn incr_sensor_db_succ(&self, sensor_id: uuid::Uuid) {
        let mut s = self.0.write().unwrap();

        s.per_sensor.get_mut(&sensor_id).unwrap().db_insert_succ += 1;
    }
}

/* ------------------------------------------------ Helper functions ------------------------------------------------------------ */

// This should be the same prefix as with the HTTP handler to ensure that event generation create the same path
pub const TOPIC_PREFIX: &str = "/api/sensors/";

pub fn mqtt_config<S: Into<String>>(id: S, transport: rumqttc::Transport) -> MqttOptions {
    let host = as_compose_service("mosquitto");

    match transport {
        rumqttc::Transport::Tcp => {
            let mut opt = MqttOptions::new(id, host, 1883);
            opt.set_keep_alive(Duration::from_secs(5));

            return opt;
        }
        rumqttc::Transport::Tls(_tls_configuration) => unimplemented!(),
        #[cfg(unix)]
        rumqttc::Transport::Unix => unimplemented!(),
        rumqttc::Transport::Ws => {
            let port = 9001;
            // Host must be a bit special due to:
            // https://github.com/bytebeamio/rumqtt/issues/808
            let mut opt = MqttOptions::new(id, format!("ws://{host}:{port}/mqtt"), port);
            opt.set_transport(rumqttc::Transport::Ws);
            opt.set_keep_alive(Duration::from_secs(5));

            return opt;
        }
        rumqttc::Transport::Wss(_tls_configuration) => unimplemented!(),
    }
}

#[derive(Debug)]
struct KeyPair {
    sensor_id: uuid::Uuid,
    api_key: Option<uuid::Uuid>,
}

// takes 'TOPIC_PREFIX<sensor_id>/<api_key>' and returns (Sensor_id::Uuid, Option<api_key::Uuid>)
fn split_topic(t: String) -> Result<KeyPair, AppError> {
    let p: Vec<&str> = t.split(TOPIC_PREFIX).collect();
    if p.len() != 2 {
        return AppError::internal(format!(
            "len of splitting topic '{}' by '{}' is not 2",
            t, TOPIC_PREFIX
        ));
    }

    let e: Vec<&str> = p[1].split("/").collect();
    match e.len() {
        1 => {
            let sensor_id = Uuid::parse_str(&e[0]);
            if sensor_id.is_err() {
                return AppError::internal(format!(
                    "failed to parse '{}' as uuid: {:?}",
                    &e[0], sensor_id
                ));
            }
            Ok(KeyPair {
                sensor_id: sensor_id.unwrap(),
                api_key: None,
            })
        }
        _ => {
            let sensor_id = Uuid::parse_str(&e[0]);
            if sensor_id.is_err() {
                return AppError::internal(format!(
                    "failed to parse '{}' as uuid: {:?}",
                    &e[0], sensor_id
                ));
            }
            let key_len = e[1].len();
            let maybe_api_key: Option<Uuid> = match key_len {
                0 if key_len < 36 => None,
                _ => {
                    let api_key = Uuid::parse_str(&e[1][..36]);
                    if api_key.is_err() {
                        return AppError::internal(format!(
                            "failed to parse '{}' as uuid: {:?}",
                            &e[1], api_key
                        ));
                    }
                    Some(api_key.unwrap())
                }
            };

            Ok(KeyPair {
                sensor_id: sensor_id.unwrap(),
                api_key: maybe_api_key,
            })
        }
    }
}

/* ------------------------------------------------ MQTT Subscriber ------------------------------------------------------------ */

///
/// Starts a WebSocket connection to a MQTT broker.
/// It subscribes on all topics with the prefix TOPIC_PREFIX.
/// Every incoming message must be to a topic in the form of 'TOPIC_PREFIX<sensor_id>/[<api_key>]'
///
/// NOTE AppState is required because we need to call the common ingest buisness logic function!
async fn start_mqtt_subscriber(
    state: AppState,
    transport: rumqttc::Transport,
    stats: Stats,
) -> Result<(), AppError> {
    let topic = format!("{}#", TOPIC_PREFIX);
    let mut reconnect_delay = 1;

    #[cfg(not(test))]
    let client_id = "sensbee-sub";
    #[cfg(test)]
    let client_id = "sensbee-sub".to_owned() + state.db.connect_options().get_database().unwrap();

    let opt = mqtt_config(client_id, transport);

    debug!("[MQTT] opts: {:?}", opt);

    let (client, mut eventloop) = AsyncClient::new(opt, 10);

    debug!("[MQTT] subscribe on '{}'", topic.clone());

    // Subscribe to all topics
    client.subscribe(topic, QoS::AtMostOnce).await?;

    info!("[MQTT] starting eventloop polling");

    loop {
        // Use recv().await which yields control properly
        match eventloop.poll().await {
            // Handle actual events
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                let start = Instant::now();

                info_span!("MQTT ingest handler", p.topic);

                stats.incr_recv();

                // Parse sensor_id and api_key.id
                let keys_res = split_topic(p.topic.clone());
                if let Err(err) = keys_res {
                    error!(
                        "[MQTT] failed to parse uuids: '{}'\ntopic: '{:?}'",
                        err, p.topic
                    );
                    stats.incr_err_parse();
                    log_event(
                        start.elapsed(),
                        state.clone(),
                        &p.topic,
                        Some(err.to_string()),
                        false,
                        None,
                    );
                    continue;
                }
                let keys = keys_res.unwrap();

                // NOTE this guarantees that all later per sensor calls have an entry to work with
                stats.incr_sensor_recv(keys.sensor_id);

                #[cfg(test)]
                let p_copy = p.payload.clone();

                // DESIGN NOTE
                /*
                    .await couples incoming requests and buisness logic processing speed together

                    If we would spawn tokio tasks we could create unmanageable backpressure when load gets higher

                    We could think about a queueing system where we create vectors of data that get ingested.
                */
                // Call ingest data buisness logic
                let db_res = ingest_data_buisness_logic(
                    keys.sensor_id,
                    keys.api_key,
                    p.payload.clone(),
                    &state,
                )
                .await;
                if let Err(err) = db_res {
                    if err.status_code() == StatusCode::UNAUTHORIZED {
                        stats.incr_sensor_err_ingest_auth(keys.sensor_id);
                    } else {
                        stats.incr_sensor_err_ingest(keys.sensor_id);
                    }

                    //#[cfg(not(test))]
                    error!("[MQTT] failed to ingest into db: '{}' ({keys:?})", err);

                    // we dont store the payload here because it might be invalid json
                    log_event(
                        start.elapsed(),
                        state.clone(),
                        &p.topic,
                        Some(err.to_string()),
                        false,
                        None,
                    );

                    continue;
                }
                let ingested = db_res.unwrap();

                #[cfg(test)]
                debug!("[MQTT] ✅ '{}' <- '{:?}'", p.topic, p_copy);

                reconnect_delay = 1;

                stats.incr_sensor_db_succ(keys.sensor_id);

                // we append the payload here because it was valid json
                log_event(
                    start.elapsed(),
                    state.clone(),
                    &p.topic,
                    None,
                    ingested,
                    Some(String::from_utf8(p.payload.clone().to_vec()).unwrap()),
                );
            }
            // Ignore all other types of packets
            Ok(Event::Incoming(_i)) => {
                if !stats.get_connected() {
                    info!("[MQTT] connected to broker");

                    stats.set_connected(true);
                    reconnect_delay = 1;
                }

                continue;
            }
            Ok(Event::Outgoing(_o)) => {
                continue;
            }
            // Handle connection errors!
            Err(e) => {
                error!("[MQTT] ❗ Error = {:?}. Retry in {}s", e, reconnect_delay);

                tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                // increase delay when errors are consecutive
                reconnect_delay = reconnect_delay * 2;
                if reconnect_delay > 30 {
                    // Something might be broken so reestablish the connection?
                    return AppError::internal("connection seems broken".to_owned());
                }

                stats.set_connected(false);
            }
        }
    }

    // Unreachable
}

fn log_event(
    dur: Duration,
    state: AppState,
    topic: &str,
    error: Option<String>,
    ingested: bool,
    payload: Option<String>,
) {
    let mut e = LogEvent::new(
        OTelData::generate(),
        dur,
        TransportProto::MQTT,
        topic.to_string(),
        match error {
            Some(_) => StatusCode::INTERNAL_SERVER_ERROR,
            None => match ingested {
                true => StatusCode::OK,
                false => StatusCode::NO_CONTENT,
            },
        },
    );
    if payload.is_some() {
        e.with_payload(payload.unwrap());
    }
    state.events.clone().unwrap().les_chan.send(e).unwrap();
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use actix_http::StatusCode;
    use log::debug;
    use rumqttc::{AsyncClient, QoS};
    use serde::Serialize;
    use serde_json::json;
    use sqlx::PgPool;
    use std::str::FromStr;
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    use crate::database::models::db_structs::DBOperation;
    use crate::handler::data_ingest::mqtt::{mqtt_config, split_topic, TOPIC_PREFIX};
    use crate::handler::models::requests::SensorDataIngestEntry;
    use crate::state::AppState;
    use crate::test_utils::tests::{
        create_test_api_keys, create_test_app, create_test_sensors, john,
    };

    pub async fn mqtt_client_publish<T>(
        sensor_id: uuid::Uuid,
        api_key: Option<uuid::Uuid>,
        payload: Option<T>,
        expected_status: StatusCode,
        state: AppState,
    ) -> anyhow::Result<()>
    where
        T: Serialize + Clone,
    {
        let opt = mqtt_config(
            "sensbee-pub-test-".to_owned() + state.db.connect_options().get_database().unwrap(),
            rumqttc::Transport::Ws,
        );

        let (client, mut eventloop) = AsyncClient::new(opt, 10);

        let t = match api_key {
            Some(key) => format!("{TOPIC_PREFIX}{sensor_id}/{key}"),
            None => format!("{TOPIC_PREFIX}{sensor_id}"),
        };

        // When this fails it indicates that the listener has not been initialized
        let m = state.mqtt_listener.clone().unwrap();
        // Save stats before we send packets
        let s_before = m.stats.clone().read_sensor_stats(&sensor_id);

        let p: Vec<u8> = match payload {
            None => vec![] as Vec<u8>,
            Some(v) => serde_json::to_vec(&v).unwrap(),
        };

        // Send the message
        client
            .publish(t.to_owned(), QoS::AtLeastOnce, false, p)
            .await
            .unwrap();
        while let Ok(notification) = eventloop.poll().await {
            match notification {
                rumqttc::Event::Incoming(packet) => {
                    match packet {
                        rumqttc::Packet::PubAck(_pub_ack) => {
                            // This confirms that the broker has recieved the packet.
                            // Now we must wait until it is recieved by our backend.
                            break;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Check for the incoming packet
        let start_time = Instant::now();
        let mut current_delay = Duration::from_millis(2);
        loop {
            tokio::time::sleep(current_delay).await;

            // Read the MQTT service stats
            let s_after = m.stats.clone().read_sensor_stats(&sensor_id);

            // Check if the sum of possible outcomes is less than to all packets indicating it has not been fully processed
            if s_before.packets_recv == s_after.packets_recv {
                debug!("Package not yet recv");
            } else if s_after.res_sum() < s_after.packets_recv {
                debug!("Package recv but not yet fully processed");
            } else {
                // Check if a result exists and ifs its the expected outcome
                if s_before.db_insert_succ + 1 == s_after.db_insert_succ {
                    if expected_status != StatusCode::OK {
                        panic!("{} != {}", expected_status, StatusCode::OK);
                    }
                    return Ok(());
                }
                if s_after.err_ingest - s_before.err_ingest > 0 {
                    if expected_status != StatusCode::INTERNAL_SERVER_ERROR {
                        panic!(
                            "{} != {}",
                            expected_status,
                            StatusCode::INTERNAL_SERVER_ERROR
                        );
                    }
                    return Ok(());
                }
                if s_after.err_ingest_auth - s_before.err_ingest_auth > 0 {
                    if expected_status != StatusCode::UNAUTHORIZED {
                        panic!("{} != {}", expected_status, StatusCode::UNAUTHORIZED);
                    }
                    return Ok(());
                }

                // Unhandeld outcome
                panic!(
                    "Pkg recv but outcome is unhandeld ({sensor_id} -> {expected_status})\nStats:\n{:?}\n{:?}",
                    s_before, s_after
                );
            }

            // When 100ms have elapsed when can be reasonably sure that something went wrong
            if start_time.elapsed() > Duration::from_millis(100) {
                panic!(
                    "packet has not been recieved after timeout ({})\nStats:\n{:?}\n{:?}",
                    start_time.elapsed().as_millis(),
                    s_before,
                    s_after
                );
            }

            // Non-linear delay increase (e.g., exponential backoff)
            current_delay = current_delay.saturating_mul(2); // Double the delay
            if current_delay > Duration::from_millis(25) {
                current_delay = Duration::from_millis(25); // Cap the delay
            }
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
    pub async fn test_mqtt(pool: PgPool) {
        let (_app, state) = create_test_app(pool).await;
        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

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
        let public_sensor = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor5")
            .unwrap();

        // --- Ingest data on a sensor with a valid api key for writing -- should work ---

        let payload = json!({
            "col1": 42,
            "col2": 56.789,
            "col3": "Hello",
        });
        let data_entry = Some(vec![SensorDataIngestEntry::from_json(payload, None)]);

        let _ = mqtt_client_publish(
            target_sensor_own.1,
            Some(api_key_write),
            data_entry.clone(),
            StatusCode::OK,
            state.clone(),
        )
        .await;

        // --- Ingest empty on a sensor with a valid api key for writing -- should fail ---

        let _ = mqtt_client_publish(
            target_sensor_own.1,
            Some(api_key_write),
            Some(()),
            StatusCode::INTERNAL_SERVER_ERROR,
            state.clone(),
        )
        .await;

        // --- Ingest data on a public sensor without api key -- should work ---

        let _ = mqtt_client_publish(
            public_sensor.1,
            None,
            data_entry.clone(),
            StatusCode::OK,
            state.clone(),
        )
        .await;

        // --- Ingest data without a proper api key -- should fail ---

        let _ = mqtt_client_publish(
            target_sensor_own.1,
            None,
            data_entry.clone(),
            StatusCode::UNAUTHORIZED,
            state.clone(),
        )
        .await;

        // --- Ingest data on a non existing sensor -- should fail ---

        let _ = mqtt_client_publish(
            Uuid::new_v4(),
            None,
            data_entry.clone(),
            StatusCode::INTERNAL_SERVER_ERROR,
            state.clone(),
        )
        .await;
    }

    #[sqlx::test(migrations = "../migrations")]
    pub async fn test_mqtt_topic_split(pool: PgPool) {
        let (_app, _state) = create_test_app(pool).await;

        let sensor_id = Uuid::from_str("5401df3b-9367-4e8b-b2dd-6dba2b6cade6").unwrap();
        let api_key = Uuid::from_str("460015cb-1c9f-46b1-bcf2-8a33fd981dcb").unwrap();

        let def_topic = format!("{TOPIC_PREFIX}{sensor_id}");
        let res = split_topic(def_topic.to_string()).unwrap();
        assert!(res.sensor_id == sensor_id);
        assert!(res.api_key.is_none());

        let def_topic_with_key = format!("{TOPIC_PREFIX}{sensor_id}/{api_key}");
        let res = split_topic(def_topic_with_key.to_string()).unwrap();
        assert!(res.sensor_id == sensor_id);
        assert!(res.api_key == Some(api_key));

        // Make sure that a trailing slash is valid
        let def_topic_with_trailing_slash = format!("{TOPIC_PREFIX}{sensor_id}/");
        let res = split_topic(def_topic_with_trailing_slash.to_string()).unwrap();
        assert!(res.sensor_id == sensor_id);
        assert!(res.api_key.is_none());

        // Make sure that arbitrary stuff as suffix is ignored
        let topic_with_bad_suffix =
            format!("{TOPIC_PREFIX}{sensor_id}/{api_key}smx/device/051001211/info");
        let res = split_topic(topic_with_bad_suffix.to_string()).unwrap();
        assert!(res.sensor_id == sensor_id);
        assert!(res.api_key == Some(api_key));
    }
}
