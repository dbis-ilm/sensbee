use std::future::{ready, Ready};

use actix_web::error::ErrorUnauthorized;
use actix_web::{dev::Payload, Error as ActixWebError};
use actix_web::{http, web, FromRequest, HttpMessage, HttpRequest};
use crate::authentication::token::verify_jwt_token;
use crate::authentication::token_cache;
use crate::handler::models::responses::ErrorResponse;
use crate::state::AppState;

pub struct JwtMiddleware {
    pub user_id: Option<uuid::Uuid>,
    pub token_id: Option<uuid::Uuid>,
}

impl FromRequest for JwtMiddleware {
    type Error = ActixWebError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let data = req.app_data::<web::Data<AppState>>().unwrap();

        // Get token from cookie or authentication header

        let token = req
            .cookie("token")
            .map(|c| c.value().to_string())
            .or_else(|| {
                req.headers()
                    .get(http::header::AUTHORIZATION)
                   // .map(|h| h.to_str().unwrap().split_at(7).1.to_string())
                   .map(|h| h.to_str().unwrap().to_string())
            });

        if token.is_none() {
            // Some api paths might accept public access
            return ready(Ok(JwtMiddleware { user_id: None, token_id: None }));
        }

        // Validate token identity (against public key) >> Expired token are removed from cache

        let access_token_details = match verify_jwt_token(
            data.jwt.public_key.to_owned(),
            &token.unwrap(),
        ) {
            Ok(token_details) => token_details,
            Err(e) => {
                let json_error = ErrorResponse {
                    status: "Invalid token!".to_string(),
                    message: format!("{:?}", e),
                };

                return ready(Err(ErrorUnauthorized(json_error)));
            }
        };

        let token_id = &access_token_details.token_uuid;

        // Check if token is still present in valid token cache

        let (valid, user_id) = token_cache::has_token(token_id.to_owned());

        if !valid {
            return ready(Err(ErrorUnauthorized(ErrorResponse {
                status: "Invalid token".to_string(),
                message: "Token has expired!".to_string(),
            })));
        }

        // At this point we do not validate the user (if he still exists), this is done in handlers

        req.extensions_mut().insert::<uuid::Uuid>(user_id.unwrap());

        ready(Ok(JwtMiddleware { user_id, token_id: Some(token_id.to_owned()) }))
    }
}
