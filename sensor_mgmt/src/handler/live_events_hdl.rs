use crate::authentication::jwt_auth;
use crate::database::models::events::LOG_EVENTS_GENERAL_CHANNEL;
use crate::features::user_sens_perm::UserSensorPerm;
use crate::handler::policy;
use crate::state::AppState;
use crate::{
    database::{
        sensor_events_db::{get_general_event_history, get_sensor_event_history},
        user_db,
    },
    utils::AppError,
};
use actix_web::web;
use actix_web::{web::Data, Error, HttpRequest, HttpResponse};
use actix_ws::AggregatedMessage;
use futures_util::StreamExt as _;
use log::error;
use serde::Deserialize;
use sqlx::postgres::PgListener;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::time::interval;
use uuid::Uuid;

/* ------------------------------------------------ WS Open Request handler ------------------------------------------------------------ */

// https://github.com/actix/examples/blob/master/websockets/chat-actorless/src/main.rs

/// Handler that opens a WebSocket connection to which all Events will be relayed
/// User must be authenticated admin!
pub async fn stream_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> Result<HttpResponse, Error> {
    // Enforce login
    if jwt.user_id.is_none() {
        AppError::unauthorized_generic()?
    }

    let (res, session, stream) = actix_ws::handle(&req, stream)?;

    tokio::task::spawn_local(ws_handler(session, stream, state, jwt.user_id.unwrap()));

    Ok(res)
}

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize)]
struct SocketMessage {
    sensor: Uuid,
}

/* ------------------------------------------------ WS handler ------------------------------------------------------------ */

/// Function for general ws message handling
async fn ws_handler(
    mut session: actix_ws::Session,
    msg_stream: actix_ws::MessageStream,
    state: Data<AppState>,
    user_id: Uuid,
) {
    log::debug!("[WS] connected");

    let mut subs: HashSet<String> = HashSet::new();

    // TOOD ??
    let mut msg_stream = msg_stream
        .max_frame_size(128 * 1024)
        .aggregate_continuations()
        .max_continuation_size(2 * 1024 * 1024);

    let user = user_db::get_user_by_id(user_id, &state).await.unwrap();
    let admin = user_db::is_admin_user(user_id, &state).await;

    // Open a listener on the DB to recieve all log_events
    let mut listener = PgListener::connect_with(&state.db).await.unwrap();

    // Connection keep alive
    let mut last_heartbeat = Instant::now();
    let mut interval = interval(HEARTBEAT_INTERVAL);

    // loop to recv/send on the websocket
    let close_reason = loop {
        tokio::select! {
            Some(Ok(msg)) = msg_stream.next() => {

                match msg {
                    AggregatedMessage::Ping(bytes) => {
                        last_heartbeat = Instant::now();
                        session.pong(&bytes).await.unwrap();
                    }
                    AggregatedMessage::Pong(_) => {
                        last_heartbeat = Instant::now();
                    }

                    AggregatedMessage::Text(text) => {
                        match serde_json::from_str::<SocketMessage>(&text) {
                            Ok(r) => {

                                // check sensor permissions
                                let perm_check = policy::require_sensor_permission(Some(user.id), r.sensor, UserSensorPerm::Read,  &state).await;
                                if perm_check.is_some() {
                                    return;
                                }

                                let channel = format!("sensor/{}", r.sensor);
                                let sensor_str = r.sensor.to_string();

                                // Check if we are already subscribed on the sensor channel
                                if subs.get(&sensor_str).is_some() {

                                    listener.unlisten(&channel).await.unwrap();

                                    subs.remove(&sensor_str);
                                    log::info!("subs updated {:?}", subs);
                                } else {
                                    // Listen to sensor channel
                                    listener.listen(&channel).await.unwrap();

                                    // Gather historical data and send it as well
                                    match get_sensor_event_history(10,state.db.clone(),r.sensor).await {
                                        Ok(past_events) => {
                                            for event in past_events {
                                                session.text(serde_json::to_string(&event.data).unwrap()).await.unwrap();
                                            }
                                        },
                                        Err(err) => error!(" get_sensor_event_history failed with: {}", err),
                                    }

                                    subs.insert(sensor_str);
                                    log::info!("Subs updated {:?}", subs);
                                }
                            },
                            Err(err) => {
                                error!("failed to deserliazie with {}", err);
                                if admin {
                                    // TODO this should be optionally invoked by the user after connection
                                    listener.listen(LOG_EVENTS_GENERAL_CHANNEL).await.unwrap();

                                    // Send general history data
                                    match get_general_event_history(state.db.clone()).await {
                                        Ok(past_events) => {
                                            for event in past_events {
                                                session.text(serde_json::to_string(&event.data).unwrap()).await.unwrap();
                                            }
                                        },
                                        Err(err) => error!(" get_sensor_event_history failed with: {}", err),
                                    }
                                }
                            },
                        };
                    }

                    AggregatedMessage::Binary(_bin) => {
                        log::warn!("unexpected binary message");
                    }

                    AggregatedMessage::Close(reason) => break reason,
                }
            }

            event = listener.recv() => {
                match event {
                    Ok(not) => {
                        session.text(not.payload()).await.unwrap();
                    },
                    Err(err) => error!("[WS] listener.recv(): {:?}", err),
                }

            }

            _ = interval.tick() => {
                if Instant::now().duration_since(last_heartbeat) > CLIENT_TIMEOUT {
                    break None;
                }
                let _ = session.ping(b"").await;
            }

            else => {
                break None;
            }
        }
    };

    // attempt to close connection gracefully
    let _ = session.close(close_reason).await;

    log::debug!("[WS] disconnected");
}
