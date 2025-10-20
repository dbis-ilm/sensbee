use sensor_mgmt::database::models::events::{
    HandlerChangeNotificaiton, LOG_EVENTS_HANDLER_CHANGED_CHANNEL,
};
use sensor_mgmt::{
    database::{
        data_chain_db, data_transformer_db, event_handler_db,
        models::{
            data_transformer::DataTransformer,
            events::{EventHandler, LogEvent},
        },
    },
    features::sensor_data_transform::{
        get_transformed_data, start_websocket_task, TransformService,
    },
};
use sqlx::postgres::PgListener;
use std::sync::LazyLock;
use std::{
    collections::HashMap,
    str::{from_utf8, FromStr},
    sync::Arc,
};
use tasks_lib::SharedState;
use tasks_lib_macro::task;
use tokio::sync::mpsc::{self, channel};
use tokio::sync::Mutex;
use tracing::{error, info, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

static CONFIG_RELOAD_CHAN: LazyLock<(
    Mutex<mpsc::Sender<HandlerChangeNotificaiton>>,
    Mutex<mpsc::Receiver<HandlerChangeNotificaiton>>,
)> = LazyLock::new(|| {
    let (sender, receiver) = channel::<HandlerChangeNotificaiton>(1);

    (Mutex::new(sender), Mutex::new(receiver))
});

/*
static CONFIG_RELOAD_CHAN: Lazy<(mpsc::Receiver<HandlerChangeNotificaiton>, mpsc::Sender<HandlerChangeNotificaiton>)> = Lazy::new(|| {
    let (sender, receiver) = channel::<HandlerChangeNotificaiton>();
});
*/

/// This task listens on the hander changed channel.
/// If a message arries on that channel we need to tell the actual event handling task about this
///
/// This split is needed because the event handler task restarts for the config reload.
/// During the restart it might loose other config change events which are captured by this task.
///
/// NOTE This does not prevent loss of sensor events during config reload!
#[task]
pub async fn event_handler_listener(
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Waiting for LOG_EVENTS_HANDLER_CHANGED_CHANNEL events");

    // Open a listener on the DB to recieve all notifications on our channel
    let mut listener = PgListener::connect_with(&state.db).await.unwrap();
    listener.listen(LOG_EVENTS_HANDLER_CHANGED_CHANNEL).await?;

    loop {
        tokio::select! {
            msg = listener.recv() => {
                match msg {
                    Ok(event) => {
                        match serde_json::from_str::<HandlerChangeNotificaiton>(event.payload()) {
                            Ok(log_event) => {
                                // Retrieve Otel context from the event
                                let parent_otel_ctx = log_event.otel.context.extract();
                                let span = info_span!(
                                    "process_log_event",
                                    event.channel = LOG_EVENTS_HANDLER_CHANGED_CHANNEL,
                                    otel.kind = "CONSUMER",
                                );
                                span.set_parent(parent_otel_ctx);
                                let _guard = span.enter();

                                // handle the event
                                info!("handle_handler_change_event: {:?}.", event);

                                let _ = CONFIG_RELOAD_CHAN
                                    .0
                                    .lock().await
                                    .send(log_event)
                                    .await
                                    .unwrap();
                            },
                            Err(err) => error!("parsing into HandlerChangeNotificaiton failed: {:?}", err),
                        }
                    },
                    Err(err) => error!("listener.recv(): {:?}", err),
                }
            }
        }
    }
}

#[task]
async fn general_events_listener_test(
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ts = Arc::new(start_websocket_task(state.db.clone()));

    // The event handling loop
    loop {
        // Subscribe on all channels that have an active handler registered
        let sensor_handler = data_chain_db::get_sensor_event_handler(&state.db).await?;
        let mut handler_map: HashMap<Uuid, Vec<(EventHandler, Option<DataTransformer>)>> =
            HashMap::new();

        let mut listener = PgListener::connect_with(&state.db).await.unwrap();
        if sensor_handler.is_some() {
            for handler in sensor_handler.unwrap() {
                let h = event_handler_db::load(handler.event_handler_id, &state.db).await?;
                let dt = match handler.data_transformer_id {
                    Some(dt_id) => Some(data_transformer_db::load(dt_id, &state.db).await?),
                    None => None,
                };
                let new_entry = (h, dt);

                // when we have seen the sensor_id already we dont need to listen on its channel
                if !handler_map.contains_key(&handler.sensor_id) {
                    listener
                        .listen(&format!("sensor/{}", handler.sensor_id))
                        .await?;
                    // TODO for some reason this appears in an otel context in the log?
                    info!("listening for events on {}", handler.sensor_id);

                    handler_map.insert(handler.sensor_id, vec![new_entry]);
                } else {
                    handler_map
                        .get_mut(&handler.sensor_id)
                        .unwrap()
                        .push(new_entry);
                }
            }
        } else {
            listener.listen("sensor/").await?;
        }

        let mut recv = CONFIG_RELOAD_CHAN.1.lock().await;

        // Wait for events
        loop {
            tokio::select! {
                msg = listener.recv() => {
                    match msg {
                        Ok(event) => {
                            match serde_json::from_str::<LogEvent>(event.payload()) {
                                Ok(log_event) => {
                                    let sensor_id = Uuid::from_str(&event.channel()[7..]).unwrap();
                                    let hdl = handler_map.get(&sensor_id).unwrap().clone();
                                    tokio::spawn(spawn_handler(state.clone(), ts.clone(), log_event,(sensor_id, hdl)));
                                },
                                Err(err) => error!("parsing into LogEvent failed: {:?}", err),
                            }
                        },
                        Err(err) => return Err(Box::new(err)),
                    }
                }
                hcn = recv.recv() => {
                    // Retrieve Otel context from the event
                    let parent_otel_ctx = hcn.unwrap().otel.context.extract();
                    let span = info_span!(
                        "reload_handler_config_event",
                        otel.kind = "CONSUMER",
                    );
                    span.set_parent(parent_otel_ctx);
                    let _guard = span.enter();

                    // handle the event
                    info!("recieved config reload message. Restarting...");
                    break;
                }
            }
        }
    }
}

async fn spawn_handler(
    _state: SharedState,
    ts: Arc<TransformService>,
    event: LogEvent,
    handler: (Uuid, Vec<(EventHandler, Option<DataTransformer>)>),
) {
    // Setup otel span
    let parent_otel_ctx = event.otel.context.extract();
    let span = info_span!(
        "process_log_event",
        channel = %handler.0,
        otel.kind = "CONSUMER",
    );
    span.set_parent(parent_otel_ctx);
    let _guard = span.enter();

    // Get all handler
    for t in handler.1 {
        let d = serde_json::to_string(&event).unwrap();
        if t.1.is_some() {
            let transformer = t.1.unwrap();
            let res = get_transformed_data(&transformer.id, d.clone(), &ts)
                .await
                .unwrap();

            let res =
                t.0.handle_event(&event, from_utf8(&res).unwrap().to_string())
                    .await;
            info!("handle event result: {:?}", res);
        } else {
            let res = t.0.handle_event(&event, d).await;

            info!("handle event result: {:?}", res);
        }
    }
}
