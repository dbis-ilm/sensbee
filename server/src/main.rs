use std::env;
use actix_web::{web::Data, http::header, App, HttpServer};
use actix_web::middleware::Logger;
use actix_cors::Cors;
use env_logger::Env;
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;

use sensor_mgmt::state::{init_app_state, JWTConfig};
use sensor_mgmt::handler as handler;

use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        sensor_mgmt::handler::main_hdl::api_base_handler,
        sensor_mgmt::handler::main_hdl::health_checker_handler,

        sensor_mgmt::handler::auth_hdl::login_user_handler,
        sensor_mgmt::handler::auth_hdl::logout_user_handler,
    
        sensor_mgmt::handler::sensor_hdl::list_sensors_handler,
        sensor_mgmt::handler::sensor_hdl::get_sensor_info_handler,
        sensor_mgmt::handler::sensor_hdl::create_sensor_handler,
        sensor_mgmt::handler::sensor_hdl::edit_sensor_handler,
        sensor_mgmt::handler::sensor_hdl::delete_sensor_handler,
        sensor_mgmt::handler::sensor_hdl::create_sensor_api_key_handler,
        sensor_mgmt::handler::sensor_hdl::delete_sensor_api_key_handler,

        sensor_mgmt::handler::data_hdl::ingest_sensor_data_handler,
        sensor_mgmt::handler::data_hdl::get_sensor_data_handler,

        sensor_mgmt::handler::user_hdl::list_users_handler,
        sensor_mgmt::handler::user_hdl::register_user_handler,
        sensor_mgmt::handler::user_hdl::verify_user_handler,
        sensor_mgmt::handler::user_hdl::get_user_info_handler,
        sensor_mgmt::handler::user_hdl::edit_user_info_handler,
        sensor_mgmt::handler::user_hdl::edit_user_security_password_handler,
        sensor_mgmt::handler::user_hdl::delete_user_handler,
        sensor_mgmt::handler::user_hdl::assign_role_handler,
        sensor_mgmt::handler::user_hdl::revoke_role_handler,

        sensor_mgmt::handler::role_hdl::list_roles_handler,
        sensor_mgmt::handler::role_hdl::create_role_handler,
        sensor_mgmt::handler::role_hdl::delete_role_handler,
    ),
    tags(
        (name = "SensBee REST API", description = "Endpoints for sensor database backend SensBee")
    ),
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> { 
    println!(" ____                 ____            \n/ ___|  ___ _ __  ___| __ )  ___  ___ \n\\___ \\ / _ \\ '_ \\/ __|  _ \\ / _ \\/ _ \\\n ___) |  __/ | | \\__ \\ |_) |  __/  __/\n|____/ \\___|_| |_|___/____/ \\___|\\___|");
    println!();
    println!("v {}", std::option_env!("CARGO_PKG_VERSION").unwrap_or("NOT_SET"));
    println!("Compiled using Rust {} on {}.", compile_time::rustc_version_str!(), compile_time::datetime_str!());

    env_logger::init_from_env(Env::default().default_filter_or("info"));
    let openapi = ApiDoc::openapi();

    dotenv().ok(); 
    let database_url = std::env::var("DATABASE_URL")
        .expect("Env var DATABASE_URL is required.");

    let pool = match PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            println!("✅ Connection to the database is successful!");
            pool
        }
        Err(err) => {
            println!("🔥 Failed to connect to the database: {:?}", err);
            std::process::exit(1);
        }
    };

    println!("🚀 Server started successfully");

    match sqlx::migrate!("../migrations")
        .run(&pool)
        .await
    {
        Ok(()) => { println!("✅ Database migration was successful!") }
        Err(err) => {
                println!("🔥 Failed to migrate database: {:?}", err);
                std::process::exit(1);
        }
    };

    let cli_port = env::var("SERVER_CLI_PORT").expect("SERVER_CLI_PORT must be provided!").parse::<u16>().unwrap();

    // Start simple server for administrative internal-only access from CLI
    let server= HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .configure(handler::cli_hdl::config)
            .wrap(Cors::default())
    }).bind(("127.0.0.1", cli_port))?;

    let _ = actix_web::rt::spawn(server.run());

    // Starts main REST server
    let rest_port = env::var("SERVER_REST_PORT").expect("SERVER_REST_PORT must be provided!").parse::<u16>().unwrap();

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
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .app_data(Data::new(init_app_state(pool.clone(), JWTConfig::init())))
            .configure(handler::main_hdl::config)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi.clone()),
            )
            .wrap(cors)
    })
    .bind(("0.0.0.0", rest_port))?
    .run()
    .await
}