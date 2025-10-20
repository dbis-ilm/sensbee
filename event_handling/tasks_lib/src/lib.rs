use linkme::distributed_slice;
use sensor_mgmt::database::models::events::LogEvent;
use sqlx::PgPool;
use std::{error::Error, sync::Arc};
use tokio::time::Duration;
use tracing::{error, info};

// --- Model Definition ---

pub struct InternalState {
    pub db: PgPool,
}

impl InternalState {
    pub fn init(db: PgPool) -> Arc<InternalState> {
        Arc::new(InternalState { db })
    }
}

pub type SharedState = Arc<InternalState>;

// --- Generic Watchdog Runner Function ---

/// A task that will be restarted upon error
pub async fn run_task(task_fn: &EventHandlerTask, state: SharedState) {
    let mut restart_delay_ms = 1;

    loop {
        match task_fn(state.clone()).await {
            Ok(_) => {
                info!("Task finished");
                break;
            }
            Err(e) => {
                error!(
                    "Task failed with error: {:?}. Restarting in {}ms...",
                    e, restart_delay_ms
                );
                tokio::time::sleep(Duration::from_millis(restart_delay_ms)).await;
                restart_delay_ms = restart_delay_ms * 2;
                if restart_delay_ms > 5000 {
                    restart_delay_ms = 5000;
                }
            }
        }
    }
}

pub type EventHandlerCallback = fn(
    &LogEvent,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static,
    >,
>;

// --- Global Registry Access ---

/// The function signature for "task"s
pub type EventHandlerTask = fn(
    SharedState,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static,
    >,
>;

#[distributed_slice]
pub static TASK_REGISTRY: [EventHandlerTask] = [..];

/// This function spawns all functions marked with "task"
pub async fn spawn_tasks(state: SharedState) {
    let mut task_handles = vec![];

    info!("Trying to spawn {} tasks", TASK_REGISTRY.len());

    for task_fn in TASK_REGISTRY {
        task_handles.push(tokio::spawn(run_task(task_fn, state.clone())));
    }
}
