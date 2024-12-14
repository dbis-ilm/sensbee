use std::env;
use actix_web::{post, web, HttpResponse, Responder};
use once_cell::sync::Lazy;
use serde_derive::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::features::cache;
use crate::handler::policy;

const CLI_ACCESS_KEY: Lazy<String> = Lazy::new(|| {env::var("CLI_ACCESS_KEY").expect("CLI_ACCESS_KEY must be provided!") });

#[post("/clear_cache")]
async fn clear_cache_handler(params: web::Query<AccessKey>) -> impl Responder {
    if !validate_cli_access(params.into_inner()) {
        return policy::unauthorized("Invalid access key!".to_string()).unwrap();
    }

    cache::purge_all();

    HttpResponse::Ok().finish()
}

// -------------------------------------------------------------------------------------------------

pub fn config(conf: &mut web::ServiceConfig) {
    let cli_scope = web::scope("/cli")
        .service(clear_cache_handler)
        ;

    conf.service(cli_scope);
}

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone)]
struct AccessKey {
    pub key: Option<String>
}

fn validate_cli_access(key: AccessKey) -> bool {
    key.key.is_some() && *CLI_ACCESS_KEY == key.key.unwrap()
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use actix_http::{Method};
    use actix_web::http::StatusCode;
    use serde_json::Value;
    use sqlx::PgPool;
    use uuid::Uuid;
    use crate::handler::cli_hdl::CLI_ACCESS_KEY;
    use crate::test_utils::tests::{create_test_app_cli, execute_request};

    #[sqlx::test(migrations = "../migrations")]
    async fn test_clear_cache(_: PgPool) {
        let app = create_test_app_cli().await;

        // --- Check access without Access key - Should fail ---
        
        let _ = execute_request("/cli/clear_cache", Method::POST,
                                None::<Value>, None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Check access with invalid Access key - Should fail ---

        let _ = execute_request(&format!("/cli/clear_cache?key={}", Uuid::new_v4().to_string()), Method::POST,
                                None::<Value>, None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Check access with valid Access key - Should succeed ---

        let _ = execute_request(&format!("/cli/clear_cache?key={}", *CLI_ACCESS_KEY), Method::POST,
                                None::<Value>, None,
                                StatusCode::OK, &app).await;
    }
}