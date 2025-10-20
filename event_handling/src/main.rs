use sensor_mgmt::features::config::database_url;
use sqlx::postgres::PgPoolOptions;
use tasks_lib::InternalState;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info};

mod tasks;

/*

Thread main : Configuration & Handler Management
    Listen for DB changes
    -> New event listener
    -> remove old, change config, etc..

    -> Event handler chill in a queue with a lock.
    When an update to the handlers needs to happen get the global lock
    then change the handler

Thread 2 : Configuration & Handler Management
    Create list of possible channels based on current handler config
    -> Only add channel that have actual handlers
    NOTE if a sensor uuid has no handler then we dont listen for events?

    Listen for Events on all channels
    -> Put them in a queue? Buffered Channel?

    TODO what happens when the config changes? Do we just drop?
    If we restart we loose all not handeld events...

    Handle new different than update?

Thread pool:
    Listen on a channel
    -> Recv event
    -> Grab a handler and lock it
    -> Run the handler
    -> Release the lock

    TODO should we somehow track which handler handeld which event?
*/

#[tokio::main]
async fn main() {
    // --- Init logger ---

    let (a, b, c) = sensor_mgmt::features::telemetry::init_telemetry("event_handling")
        .await
        .unwrap();

    // --- Init database connection ---
    let database_url = database_url();
    let pool = match PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            info!("✅ Connection to the database is successful!");
            pool
        }
        Err(err) => {
            error!("🔥 Failed to connect to the database: {:?}", err);
            std::process::exit(1);
        }
    };

    let s = InternalState::init(pool);

    // --- Spawn all the tasks ---

    tasks_lib::spawn_tasks(s).await;

    // --- Graceful Shutdown ---

    #[cfg(unix)]
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to create SIGTERM listener");
    #[cfg(not(unix))]
    let sigterm_future = std::future::pending();

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("\nCtrl+C received!");
        }
        _ = sigterm.recv() => {
            info!("\nSIGTERM received!");
        }
    }

    let _ = sensor_mgmt::features::telemetry::stop_telemetry(a, b, c);

    info!("stopped");
}
