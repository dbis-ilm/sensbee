use actix_http::StatusCode;
use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgQueryResult;
use sqlx::{FromRow, PgPool};
use std::fmt;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::handler::models::requests::TransportProto;
use crate::handler::models::telelmetry::{OTelData, PropagationContext};
use crate::utils::uuid_schema;

pub const LOG_EVENTS_GENERAL_CHANNEL: &str = "log_events";

/// A structure holding all relevant data for a LogEvent
#[derive(Serialize, Deserialize, FromRow)]
pub struct LogEvent {
    // Tracing data
    pub otel: OTelData,

    // Timing data
    pub t: NaiveDateTime,
    pub dur: Duration,

    // Common event data
    pub proto: TransportProto,
    pub path: String,
    pub status: u16,

    // Some optional fields
    pub payload: Option<String>,
}

impl LogEvent {
    pub fn new(
        otel: OTelData,
        dur: Duration,
        proto: TransportProto,
        path: String,
        status: StatusCode,
    ) -> Self {
        LogEvent {
            otel,
            t: Utc::now().naive_utc(),
            dur,
            proto,
            path,
            status: status.as_u16(),
            payload: None,
        }
    }

    pub fn with_payload(&mut self, v: String) -> &Self {
        self.payload = Some(v);
        self
    }
}

impl fmt::Debug for LogEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogEvent")
            .field("time", &self.t)
            .field("dur", &self.dur)
            .field("proto", &self.proto)
            .field("path", &self.path)
            .field("status", &self.status)
            .finish()
    }
}

/// The DB representation of a LogEvent
/// As most are sensor centric, the sensor_id is saved to a column for easier queriy of relevant past events.
/// As the structure of the LogEvent might change over time, the event itself is stored as schemaless json.
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct LogEventRow {
    pub t: DateTime<Utc>,
    pub sensor_id: Option<Uuid>,
    pub data: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventFilter {
    proto: TransportProto, // Map of protos?
    handler: u32,          // Map of handlers?
    sensor: u32,           // Map of sensor uuids?
    result_status: u32,    // Map of result status_codes?
}

pub struct EventEngineState {
    // Logging & Event Service Channel
    pub les_chan: UnboundedSender<LogEvent>,
}

/*

TODO

1. API Pfade so anpassen das Dinge sich unter Sensoren gruppieren -> Events nehmen dann /api/sensor/{id} umd sensor events zu generieren

2. MQTT topic sollte /api/sensor/{id} sein damit man das nicht extra behandenln muss?

2. Log Events mehr sensor zentrisch gestalten?

3. Wenn event kommt dann checken ob auf dem sensor handler existieren
-> dann filter anwenden
-> Event handler ausführen (sind jetzt callbacks)
-> callbacks brauchen METHOD (GET, POST, etc), URL, PARAMS <String,String>, HEADER <String,String>
-> Dann wird dahin [{},...] die events geschickt

4. filter einbauen

5. Event handler deaktivieren
- Nur explizit löschen

*/

/// An EventHandler has a filter, a script and a state.
/// Once an Event is fired each Handlers filter is checked for matches.
/// If a match is found the script gets run with the state and the event as input.
#[derive(Debug, Serialize, Deserialize, Clone, FromRow, ToSchema)]
pub struct EventHandler {
    #[schema(schema_with = uuid_schema)]
    pub id: Uuid,
    pub name: String,

    // Filter config
    #[sqlx(default)]
    pub filter: String,

    // webhook settings
    #[sqlx(default)]
    pub url: String,
    #[sqlx(default)]
    pub method: String, // converted to http::Method

                        // TODO maybe add some more meta info?
                        // created_at, updated_at, version
}

impl EventHandler {
    pub fn new(url: String) -> EventHandler {
        EventHandler {
            id: Uuid::new_v4(),
            name: "".to_string(),
            filter: "".to_string(),
            url,
            method: "".to_string(),
        }
    }

    ///
    fn match_filter(&self, _event: &LogEvent) -> bool {
        // TODO
        return true;
    }

    ///
    pub async fn handle_event(&self, event: &LogEvent, body: String) -> anyhow::Result<()> {
        // Check if a transformer is needed

        info!("handlerCfg {:?} and body: {}", self, body);

        // check if the log event shall be handeld by this handler
        if !self.match_filter(event) {
            return Ok(());
        }

        // TODO make this less primitive...

        // Run hook
        info!("Event: {:?}", event);

        let client = reqwest::Client::new();
        if self.method != "GET" {
            let res = client
                .post(self.url.clone())
                .body(body)
                .header("Content-Type", "application/json")
                .send()
                .await?;

            info!("status {:?} body {:?}", res.status(), res.text().await);
        } else {
            let res = client.get(self.url.clone()).send().await?;
            info!("res: {res:?}");
        }

        Ok(())
    }
}

///
/// Signal to the event_handler that the config has changed
///

pub const LOG_EVENTS_HANDLER_CHANGED_CHANNEL: &str = "log_events_handler";
#[derive(Debug, Serialize, Deserialize)]
pub struct HandlerChangeNotificaiton {
    pub otel: OTelData,
}

/// Once a handler change is made we need to inform the handler service that it needs to update the config
pub async fn signal_handler_change(db: &PgPool) -> Result<PgQueryResult, sqlx::Error> {
    let otel_ctx = Span::current().context();

    let d = HandlerChangeNotificaiton {
        otel: OTelData {
            context: PropagationContext::inject(&otel_ctx),
        },
    };

    return sqlx::query("SELECT pg_notify($1, $2)")
        .bind(LOG_EVENTS_HANDLER_CHANGED_CHANNEL)
        .bind(serde_json::to_string(&d).unwrap())
        .execute(db)
        .await;
}
