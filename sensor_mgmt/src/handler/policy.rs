use actix_web::HttpResponse;
use serde_json::json;
use crate::features::cache;
use crate::database::sensor_db;
use crate::database::user_db::is_admin_user;
use crate::features::user_sens_perm::UserSensorPerm;
use crate::state::AppState;

/// Checks, if the user is logged in and verified or returns a Http error response.
pub async fn require_login(user_id: Option<uuid::Uuid>, state: &AppState) -> Option<HttpResponse> {
    if user_id.is_none() {
        return unauthorized("You are not logged in, please provide token".to_string());
    }

    let user_id = user_id.unwrap();
    
    let user = cache::request_user(user_id, state).await;
    
    if user.is_none() {
        println!("Failed access with token of non-existent user {}!", user_id);
        return unauthorized("User does not exists!".to_string());
    }

    if !user.unwrap().verified {
        println!("Failed access with token of non-verified user {}!", user_id);
        return unauthorized("User has not been verified!".to_string());
    }

    None
}

/// Checks, if the user is a logged in admin user or returns a Http error response.
pub async fn require_admin(user_id: Option<uuid::Uuid>, state: &AppState) -> Option<HttpResponse> {
    let login_check = require_login(user_id, &state).await;

    if login_check.is_some() {
        return login_check;
    }

    let user_id = user_id.clone().unwrap();

    match is_admin_user(user_id, &state).await {
        false => {
            println!("Failed admin access with token of non-admin user {}!", user_id);
            unauthorized("No admin permissions!".to_string())
        },
        true => None
    }
}

/// Checks, if the user (if valid and verified login) or guest has the specified permissions for the sensor or returns a Http error response.
pub async fn require_sensor_permission(user_id: Option<uuid::Uuid>, sensor_id: uuid::Uuid, perm: UserSensorPerm, state: &AppState) -> Option<HttpResponse> {
    // Extracts user id if login is valid or None (Guest)
    
    let login_id = require_login(user_id, &state).await.map_or(user_id, |_| None);

    let sensor = cache::request_sensor(sensor_id, &state).await?;
    
    let permissions = sensor_db::get_user_sensor_permissions(login_id, &sensor, &state).await;

    match permissions.has(perm) {
        true => None,
        false => unauthorized("No permissions to perform operation on sensor!".to_string())
    }
}

pub fn unauthorized(error_msg: String) -> Option<HttpResponse> {
    Some(HttpResponse::Unauthorized().json(json!({
            "error": error_msg,
        })))
}