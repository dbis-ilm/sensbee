use actix_web::{App, HttpServer};
use actix_cors::Cors;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;


#[derive(OpenApi)]
#[openapi(
    paths(
        sensor_mgmt::handler::main_hdl::api_base_handler,
        sensor_mgmt::handler::main_hdl::health_checker_handler,

        sensor_mgmt::handler::auth_hdl::login_user_handler,
        sensor_mgmt::handler::auth_hdl::logout_user_handler,
        sensor_mgmt::handler::auth_hdl::openid_available_auth_handler,
        sensor_mgmt::handler::auth_hdl::openid_auth_callback_handler,
    
        sensor_mgmt::handler::sensor_hdl::list_sensors_handler,
        sensor_mgmt::handler::sensor_hdl::get_sensor_info_handler,
        sensor_mgmt::handler::sensor_hdl::create_sensor_handler,
        sensor_mgmt::handler::sensor_hdl::edit_sensor_handler,
        sensor_mgmt::handler::sensor_hdl::delete_sensor_handler,
        sensor_mgmt::handler::sensor_hdl::create_sensor_api_key_handler,
        sensor_mgmt::handler::sensor_hdl::delete_sensor_api_key_handler,
        sensor_mgmt::handler::sensor_hdl::load_data_chain_handler,
        sensor_mgmt::handler::sensor_hdl::set_data_chain_handler,
        sensor_mgmt::handler::sensor_hdl::delete_data_chain_handler,

        sensor_mgmt::handler::data_transform_hdl::list_data_transformer_handler,
        sensor_mgmt::handler::data_transform_hdl::load_data_transformer_handler,
        sensor_mgmt::handler::data_transform_hdl::create_data_transformer_handler,
        sensor_mgmt::handler::data_transform_hdl::update_data_transformer_handler,
        sensor_mgmt::handler::data_transform_hdl::delete_data_transformer_handler,

        sensor_mgmt::handler::event_handler_hdl::list_event_handler_handler,
        sensor_mgmt::handler::event_handler_hdl::load_event_handler_handler,
        sensor_mgmt::handler::event_handler_hdl::delete_event_handler_handler,
        sensor_mgmt::handler::event_handler_hdl::create_event_handler_handler,

        sensor_mgmt::handler::data_ingest::http::ingest_sensor_data_handler,
        sensor_mgmt::handler::data_hdl::delete_sensor_data_handler,
        sensor_mgmt::handler::data_hdl::get_sensor_data_handler,

        sensor_mgmt::handler::user_hdl::list_users_handler,
        sensor_mgmt::handler::user_hdl::register_user_handler,
        sensor_mgmt::handler::user_hdl::verify_user_handler,
        sensor_mgmt::handler::user_hdl::get_user_info_handler,
        sensor_mgmt::handler::user_hdl::edit_user_info_handler,
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
    println!("v {} OpenAPI documentation provider", std::option_env!("CARGO_PKG_VERSION").unwrap_or("NOT_SET"));
    println!("Compiled using Rust {} on {}.", compile_time::rustc_version_str!(), compile_time::datetime_str!());
    
    let openapi = ApiDoc::openapi();

    HttpServer::new(move || {
        App::new()
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi.clone()),
            )
            .wrap(Cors::default())
    })
    .bind(("0.0.0.0", 80))?
    .run()
    .await
}