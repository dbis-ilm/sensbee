use actix_cors::Cors;
use actix_web::web::PayloadConfig;
use actix_web::{http::header, web::Data, App, HttpServer};
use sensor_mgmt::features::config::{
    database_url, get_host, get_ingest_max_size_kb, get_port, inside_compose_stack,
};
use sensor_mgmt::features::event_generation::EventGenerator;
use sensor_mgmt::features::telemetry::{init_telemetry, stop_telemetry};
use sensor_mgmt::state::{development_setup, init_app_state};
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info};
use tracing_actix_web::TracingLogger;

/* -------------- Server ---------------- */

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!(" ____                 ____            \n/ ___|  ___ _ __  ___| __ )  ___  ___ \n\\___ \\ / _ \\ '_ \\/ __|  _ \\ / _ \\/ _ \\\n ___) |  __/ | | \\__ \\ |_) |  __/  __/\n|____/ \\___|_| |_|___/____/ \\___|\\___|");
    println!();
    println!(
        "v {}",
        std::option_env!("CARGO_PKG_VERSION").unwrap_or("NOT_SET")
    );
    println!(
        "Compiled using Rust {} on {}.",
        compile_time::rustc_version_str!(),
        compile_time::datetime_str!()
    );
    if !inside_compose_stack() {
        println!("Expecting services on localhost!");
    }

    let _prov = init_telemetry("sensbee").await.unwrap();

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

    match sqlx::migrate!("../migrations").run(&pool).await {
        Ok(()) => {
            info!("✅ Database migration was successful!")
        }
        Err(err) => {
            error!("🔥 Failed to migrate database: {:?}", err);
            std::process::exit(1);
        }
    };

    info!("🚀 Server started successfully");

    // Starts main REST server
    let shared_state = init_app_state(pool.clone());

    // When started in dev mode we need to setup the development login handler
    let _ = development_setup(&shared_state).await;

    let host = get_host(&shared_state.cfg);
    let port = get_port(&shared_state.cfg);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_methods(vec!["GET", "POST", "DELETE"])
            .allowed_headers(vec![
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::ACCEPT,
            ])
            .supports_credentials()
            .allow_any_origin();
        App::new()
            .wrap(EventGenerator)
            .wrap(TracingLogger::default())
            .app_data(Data::new(shared_state.clone()))
            .app_data(PayloadConfig::new(get_ingest_max_size_kb(
                &shared_state.cfg,
            )))
            .configure(sensor_mgmt::handler::main_hdl::config)
            .wrap(cors)
    })
    .bind((host, port))?
    .run()
    .await

    // TODO this should be called on server stop, but apperently there is no way to hook into server stop...
    //stop_telemetry(prov.0, prov.1, prov.2)??????
}
