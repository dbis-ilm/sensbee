use crate::authentication::{jwt_auth, token, token_cache};
use actix_web::{get, post, web, HttpResponse, Responder};
use actix_web::cookie::Cookie;
use actix_web::cookie::time::Duration as ActixWebDuration;
use serde_json::json;
use crate::database::user_db::check_user_login;
use crate::handler::policy::unauthorized;
use crate::handler::models::requests::LoginUserRequest;
use crate::state::AppState;

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body(
        content_type = "application/json",
        content = LoginUserRequest,
        description = "Credentials of the user to login.",
    ),
    tag = "Authentication",
    responses(
        (status = 200, description= "Returns the authentication token.", body = String),
        (status = 401, description= "Returns an unauthorized error if the credentials were invalid."),
    )
)]

#[post("/login")]
async fn login_user_handler(body: web::Json<LoginUserRequest>, data: web::Data<AppState>) -> impl Responder {
    let result = check_user_login(body.into_inner(), &data).await;
    
    if let Err(_) = result {
        return HttpResponse::Unauthorized().json(json!({"status": "fail", "message": "Invalid email or password"}));
    }
    
    let user = result.unwrap();
    
    if !user.verified {
        return unauthorized("User has not been verified yet!".to_string()).unwrap();
    }

    let access_token_details = match token::generate_jwt_token(
        user.id,
        data.jwt.max_age,
        data.jwt.private_key.to_owned(),
    ) {
        Ok(token_details) => token_details,
        Err(e) => {
            return HttpResponse::BadGateway().json(json!({"status": "fail", "message": format_args!("{}", e)}));
        }
    };
    
    let token = access_token_details.token.to_owned().unwrap();

    let cookie = Cookie::build("token", token.clone())
        .path("/")
        .max_age(ActixWebDuration::new(60 * data.jwt.max_age, 0))
        .http_only(true)
        .finish();

    token_cache::register_token(access_token_details.token_uuid, access_token_details);

    HttpResponse::Ok()
        .cookie(cookie)
        .json(json!(token))
}

#[utoipa::path(
    get,
    path = "/auth/logout",
    tag = "Authentication",
    responses(
        (status = 200, description= "Returns ok on successful logout."),
    )
)]

#[get("/logout")]
async fn logout_user_handler(jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    if jwt.token_id.is_some() {
        token_cache::unregister_token(jwt.token_id.unwrap());
    }

    let cookie = Cookie::build("token", "")
        .path("/")
        .max_age(ActixWebDuration::new(-1, 0))
        .http_only(true)
        .finish();

    HttpResponse::Ok()
        .cookie(cookie)
        .json(json!({"status": "success"}))
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use actix_http::Method;
    use actix_web::http::StatusCode;
    use serde_json::Value;
    use super::*;
    use sqlx::PgPool;
    use crate::test_utils::tests::{create_test_app, execute_request, jane, john, login};

    #[sqlx::test(migrations = "../migrations", fixtures("users"))]
    async fn test_login(pool: PgPool) {
        let (app, _) = create_test_app(pool).await;

        // --- Login with non-existent user - Should fail ---

        let payload = json!({
            "email": "non@existent.com",
            "password": "1234",
        });

        let _ = execute_request("/auth/login", Method::POST,
                                Some(payload), None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Login with wrong password - Should fail ---

        let payload = json!({
            "email": &john().email,
            "password": "1234",
        });

        let _ = execute_request("/auth/login", Method::POST,
                                Some(payload), None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Login with non-verified user - Should fail ---

        let payload = json!({
            "email": &jane().email,
            "password": jane().password,
        });

        let _ = execute_request("/auth/login", Method::POST,
                                   Some(payload), None,
                                   StatusCode::UNAUTHORIZED, &app).await;

        // --- Login with correct credentials - Should succeed ---

        let _ = login(&john().email, &john().password, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users"))]
    async fn test_logout(pool: PgPool) {
        let (app, _) = create_test_app(pool).await;
        
        // Logout with logged-in user --- Should succeed
        
        let token = login(&john().email, &john().password, &app).await;
        let token_details = token_cache::get_token_by_string(token.to_owned()).unwrap();

        let _ = execute_request("/auth/logout", Method::GET,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::OK, &app).await;
        
        // Check if token is removed from cache
        
        assert!(!token_cache::has_token(token_details.token_uuid).0);

        // Logout again, should fail since token is invalidated --- Should fail

        let _ = execute_request("/auth/logout", Method::GET,
                                None::<Value>, Some(token),
                                StatusCode::UNAUTHORIZED, &app).await;

        // Logout again without token --- Should succeed

        let _ = execute_request("/auth/logout", Method::GET,
                                None::<Value>, None,
                                StatusCode::OK, &app).await;
    }
}