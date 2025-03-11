use actix_web::http::header;
use actix_web::{get, web, HttpResponse, Responder};
use serde_json::json;
use serde::Serialize;
use crate::handler::{auth_hdl, role_hdl, sensor_hdl, user_hdl, data_hdl};
use crate::handler::models::responses::HealthResponse;

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

    HttpResponse::Ok().json(HealthResponse {status: "success".to_string(), message: MESSAGE.to_string()})
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
pub fn send_result<T>(result: &Result<T, anyhow::Error>) -> HttpResponse where T: Serialize {
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
            eprintln!("{}", format!("{:?}", e));
            
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

        .service(data_hdl::ingest_sensor_data_handler)
        .service(data_hdl::get_sensor_data_handler)
    
        .service(role_hdl::create_role_handler)
        .service(role_hdl::delete_role_handler)
        .service(role_hdl::list_roles_handler)

        .service(user_hdl::list_users_handler)
        .service(user_hdl::register_user_handler)
        .service(user_hdl::verify_user_handler)
        .service(user_hdl::get_user_info_handler)
        .service(user_hdl::edit_user_info_handler)
        .service(user_hdl::edit_user_security_password_handler)
        .service(user_hdl::delete_user_handler)
        .service(user_hdl::revoke_role_handler)
        .service(user_hdl::assign_role_handler)
       ;

    conf.service(scope);

    let auth_scope = web::scope("/auth")
        .service(auth_hdl::login_user_handler)
        .service(auth_hdl::logout_user_handler)
        ;

    conf.service(auth_scope);
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use actix_http::Method;
    use crate::state::init_app_state;
    use actix_web::{test, App};
    use actix_web::http::StatusCode;
    use serde_json::Value;
    use super::*;
    use sqlx::PgPool;
    use crate::state::JWTConfig;
    use crate::test_utils::tests::{create_test_app, execute_request};

    #[sqlx::test(migrations = "../migrations")]
    async fn test_health_check(pool: PgPool) {
        let (app, _) = create_test_app(pool).await;

        let body = execute_request("/api/healthchecker", Method::GET, None,
                                None::<Value>, None,
                                StatusCode::OK, &app).await;

        let resp: HealthResponse = serde_json::from_value(body).unwrap();

        assert!(resp.status == "success" && resp.message == "Smart City Database Backend");
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_config(pool: PgPool) {
        let state =  init_app_state(pool, JWTConfig::init());

        let app = App::new().app_data(web::Data::new(state.clone())).configure(config);

        let _ = test::init_service(app).await;
    }
}