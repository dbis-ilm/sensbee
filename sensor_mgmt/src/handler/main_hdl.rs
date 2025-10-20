use crate::handler::data_transform_hdl;
use crate::handler::models::responses::HealthResponse;
use crate::handler::{
    auth_hdl, data_hdl, data_ingest::http, live_events_hdl::stream_handler, role_hdl, sensor_hdl,
    user_hdl,
};
use actix_web::http::header;
use actix_web::{get, web, HttpResponse, Responder};
use serde::Serialize;
use serde_json::json;
use tracing::error;

use super::event_handler_hdl;

#[utoipa::path(
    get,
    path = "/api/healthchecker",
    tag = "System",
    responses(
        (status = 200, description= "Return I'm alive message", body = HealthResponse),
    )
)]
#[get("/healthchecker")]
async fn health_checker_handler() -> impl Responder {
    const MESSAGE: &str = "Smart City Database Backend";

    HttpResponse::Ok().json(HealthResponse {
        status: "success".to_string(),
        message: MESSAGE.to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/api/",
    tag = "System",
    responses(
        (status = 200, description= "Always returns `200 OK` as long as the server is running. This is useful for verifying the base URL of a REST JSON API, such as when configuring integrations in tools like Grafana."),
    )
)]
#[get("/")]
async fn api_base_handler() -> impl Responder {
    HttpResponse::Ok()
}

/* ------------------------------------------------Helper ------------------------------------------------------------ */

/// Sends the successful result or an error message.
pub fn send_result<T>(result: &Result<T, anyhow::Error>) -> HttpResponse
where
    T: Serialize,
{
    match result {
        Ok(res) => {
            let mut b = HttpResponse::Ok();
            b.insert_header(header::ContentType::json());

            let data = serde_json::to_value(&res).unwrap_or_default();
            if data.is_null() {
                b.body("{}")
            } else {
                b.body(data.to_string())
            }
        }
        Err(e) => {
            error!("{}", format!("{:?}", e));

            // Send the error - Error message should not reveal sensitive information!
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

pub fn config(conf: &mut web::ServiceConfig) {
    let scope = web::scope("/api")
        .service(api_base_handler)
        .service(health_checker_handler)
        .service(sensor_hdl::list_sensors_handler)
        .service(sensor_hdl::get_sensor_info_handler)
        .service(sensor_hdl::create_sensor_handler)
        .service(sensor_hdl::edit_sensor_handler)
        .service(sensor_hdl::delete_sensor_handler)
        .service(sensor_hdl::create_sensor_api_key_handler)
        .service(sensor_hdl::delete_sensor_api_key_handler)
        .service(sensor_hdl::load_data_chain_handler)
        .service(sensor_hdl::set_data_chain_handler)
        .service(sensor_hdl::delete_data_chain_handler)
        .service(data_transform_hdl::list_data_transformer_handler)
        .service(data_transform_hdl::load_data_transformer_handler)
        .service(data_transform_hdl::create_data_transformer_handler)
        .service(data_transform_hdl::update_data_transformer_handler)
        .service(data_transform_hdl::delete_data_transformer_handler)
        .service(event_handler_hdl::list_event_handler_handler)
        .service(event_handler_hdl::load_event_handler_handler)
        .service(event_handler_hdl::create_event_handler_handler)
        .service(event_handler_hdl::delete_event_handler_handler)
        .service(http::ingest_sensor_data_handler)
        .service(data_hdl::get_sensor_data_handler)
        .service(data_hdl::delete_sensor_data_handler)
        .service(role_hdl::create_role_handler)
        .service(role_hdl::delete_role_handler)
        .service(role_hdl::list_roles_handler)
        .service(user_hdl::list_users_handler)
        .service(user_hdl::register_user_handler)
        .service(user_hdl::verify_user_handler)
        .service(user_hdl::get_user_info_handler)
        .service(user_hdl::edit_user_info_handler)
        .service(user_hdl::delete_user_handler)
        .service(user_hdl::revoke_role_handler)
        .service(user_hdl::assign_role_handler)
        .service(web::resource("/les/v1/stream/ws").route(web::get().to(stream_handler)));

    conf.service(scope);

    let auth_scope = web::scope("/auth")
        .service(auth_hdl::login_user_handler)
        .service(auth_hdl::logout_user_handler)
        .service(auth_hdl::openid_available_auth_handler)
        .service(auth_hdl::openid_auth_callback_handler);

    conf.service(auth_scope);
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::state::init_app_state;
    use crate::test_utils::tests::{create_test_app, execute_request};
    use actix_http::Method;
    use actix_web::http::StatusCode;
    use actix_web::{test, App};
    use serde_json::Value;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "../migrations")]
    async fn test_health_check(pool: PgPool) {
        let (app, _) = create_test_app(pool).await;

        let body = execute_request(
            "/api/healthchecker",
            Method::GET,
            None,
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;

        let resp: HealthResponse = serde_json::from_value(body).unwrap();

        assert!(resp.status == "success" && resp.message == "Smart City Database Backend");
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_config(pool: PgPool) {
        let state = init_app_state(pool);

        let app = App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(config);

        let _ = test::init_service(app).await;
    }
}
