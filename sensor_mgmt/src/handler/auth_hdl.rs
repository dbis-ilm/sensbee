use crate::authentication::openid::{list_available_idp, retrieve_user_info};
use crate::authentication::token::{generate_jwt_token, COOKIE_NAME};
use crate::authentication::{jwt_auth, token_cache};
use crate::database::user_db::get_user_by_email;
use crate::features::config::{get_external_sbmi_host, is_prod_mode, root_user_email};
use crate::handler::main_hdl;
use crate::handler::models::responses::{AuthResponse, LoginResponse};
use crate::state::AppState;
use actix_web::cookie::time::Duration as ActixWebDuration;
use actix_web::cookie::Cookie;
use actix_web::{get, post, web, HttpResponse, Responder};
use serde_json::json;
use tracing::error;

const AUTH_COMMON_TAG: &str = "Authentication";

#[utoipa::path(
    post,
    path = "/auth/dev/login",
    tag = AUTH_COMMON_TAG,
    responses(
        (status = 200, description= "Returns the authentication token for the root user.", body = LoginResponse),
        (status = 401, description= "Returns an unauthorized error if the config is not set up correctly."),
    )
)]
#[post("/dev/login")]
async fn login_user_handler(data: web::Data<AppState>) -> impl Responder {
    // Disabled in prod, shouldnt even be registered?
    if is_prod_mode(&data.cfg) {
        return HttpResponse::Unauthorized().json(json!({"error": "feature disabled"}));
    }

    // In dev mode we return a token for the root user
    let root_email = match root_user_email(&data) {
        Some(v) => v,
        None => return HttpResponse::Unauthorized().json(json!({"error": "no root user set"})),
    };

    let user = match get_user_by_email(&root_email, &data).await {
        Ok(v) => match v {
            Some(u) => u,
            None => todo!(),
        },
        Err(err) => {
            error!("get_user_by_email failed with {}", err);
            return HttpResponse::Unauthorized()
                .json(json!({"error": "internal error while fetching root user"}));
        }
    };

    let access_token_details =
        match generate_jwt_token(user.id, data.jwt.max_age, &data.jwt.private_key) {
            Ok(token_details) => token_details,
            Err(e) => {
                return HttpResponse::BadGateway().json(json!({"error": format_args!("{}", e)}));
            }
        };

    let token = access_token_details.token.to_owned().unwrap();

    let cookie = Cookie::build(COOKIE_NAME, token.clone())
        .path("/")
        .max_age(ActixWebDuration::new(60 * data.jwt.max_age, 0))
        .http_only(true)
        .finish();

    token_cache::register_token(access_token_details.token_uuid, access_token_details);

    HttpResponse::Ok()
        .cookie(cookie)
        .json(json!({"jwt": token}))
}

#[utoipa::path(
    get,
    path = "/auth/logout",
    tag = AUTH_COMMON_TAG,
    responses(
        (status = 200, description= "Returns ok on successful logout."),
    )
)]
#[get("/logout")]
async fn logout_user_handler(jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    if jwt.token_id.is_some() {
        token_cache::unregister_token(jwt.token_id.unwrap());
    }

    let cookie = Cookie::build(COOKIE_NAME, "")
        .path("/")
        .max_age(ActixWebDuration::new(-1, 0))
        .http_only(true)
        .finish();

    HttpResponse::Ok().cookie(cookie).json("{}")
}

//
// --- OpenID Connect
//

const AUTH_OPENID_TAG: &str = "Authentication / OpenID Connect";

#[utoipa::path(
    get,
    path = "/auth/openid/list_idps",
    tag = AUTH_OPENID_TAG,
    responses(
        (status = 200, description= "Returns a list of all successfully configured OpenID endpoints."),
    )
)]
#[get("/openid/list_idps")]
async fn openid_available_auth_handler(data: web::Data<AppState>) -> impl Responder {
    main_hdl::send_result(&Ok(list_available_idp(&data).await))
}

#[utoipa::path(
    get,
    path = "/auth/openid/callback",
    tag = AUTH_OPENID_TAG,
    responses(
        (status = 200, description= "Called by IDP after authentication."),
    )
)]
#[get("/openid/callback")]
async fn openid_auth_callback_handler(
    data: web::Data<AppState>,
    params: web::Query<AuthResponse>,
) -> impl Responder {
    let res = retrieve_user_info(params.into_inner(), &data).await;
    if let Err(err) = res {
        error!("retrieve_user_info failed with: {:?}", err);
        return HttpResponse::Unauthorized().json(json!({"error": "Authentication failed"}));
    }

    let user = res.unwrap();
    if !user.verified {
        return HttpResponse::Unauthorized()
            .json(json!({"error": "User has not been verified yet"}));
    }

    let access_token_details =
        match generate_jwt_token(user.id, data.jwt.max_age, &data.jwt.private_key) {
            Ok(token_details) => token_details,
            Err(e) => {
                return HttpResponse::BadGateway().json(json!({"error": format_args!("{}", e)}));
            }
        };

    let token = access_token_details.token.to_owned().unwrap();

    let cookie = Cookie::build(COOKIE_NAME, token.clone())
        .path("/")
        .max_age(ActixWebDuration::new(60 * data.jwt.max_age, 0))
        .http_only(true)
        .finish();

    token_cache::register_token(access_token_details.token_uuid, access_token_details);

    // TODO this should redirect to the calling SBMI instead of localhost...

    HttpResponse::TemporaryRedirect()
        .cookie(cookie)
        .insert_header((
            "Location",
            format!("{}?jwt={}", get_external_sbmi_host(&data.cfg), token),
        ))
        .finish()
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{create_test_app, execute_request, john, login};
    use actix_http::Method;
    use actix_web::http::StatusCode;
    use serde_json::Value;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "../migrations", fixtures("users"))]
    async fn test_logout(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        // Logout with logged-in user --- Should succeed

        let token = login(&john(), &state).await;
        let token_details = token_cache::get_token_by_string(token.to_owned()).unwrap();

        let _ = execute_request(
            "/auth/logout",
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;

        // Check if token is removed from cache

        assert!(!token_cache::has_token(token_details.token_uuid).0);

        // Logout again, should fail since token is invalidated --- Should fail

        let _ = execute_request(
            "/auth/logout",
            Method::GET,
            None,
            None::<Value>,
            Some(token),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // Logout again without token --- Should succeed

        let _ = execute_request(
            "/auth/logout",
            Method::GET,
            None,
            None::<Value>,
            None,
            StatusCode::OK,
            &app,
        )
        .await;
    }
}
