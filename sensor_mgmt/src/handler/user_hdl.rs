use actix_web::{delete, get, post, web, Responder};
use crate::database::{role_db, user_db};
use crate::database::user_db::{is_admin_user, register_user};
use crate::handler::{main_hdl, policy};
use crate::authentication::jwt_auth;
use crate::database::models::user::UserInfo;
use crate::features::cache;
use crate::handler::models::requests::RegisterUserRequest;
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/api/users/list",
    tag = "Users",
    responses(
        (status = 200, description= "Returns a list of registered users in the system.", body = Vec<UserInfo>),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
    ),
    security(("JWT" = [])),
)]

#[get("/users/list")]
async fn list_users_handler(state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    // Only admins can list users
    
    let user_id = jwt.user_id;
    
    let admin_check = policy::require_admin(user_id, &state).await;

    if admin_check.is_some() {
        return admin_check.unwrap();
    }

    let result = user_db::user_list(&state).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    post,
    path = "/api/users/register",
    request_body(
        content_type = "application/json",
        content = RegisterUserRequest,
        description = "Details of the user to register.",
    ),
    tag = "Users",
    responses(
        (status = 200, description= "Returns the uuid of the created user.", body = String, example = json!(uuid::Uuid::new_v4().to_string())),
        (status = 500, description= "Returns an error if the registration failed."),
    )
)]

#[post("/users/register")]
async fn register_user_handler(body: web::Json<RegisterUserRequest>, data: web::Data<AppState>) -> impl Responder {
    let result = register_user(body.into_inner(), false, &data).await;
    
    if result.is_err() {
        main_hdl::send_result(&result)
    } else { // Only send user_id back
        main_hdl::send_result(&Ok::<uuid::Uuid, anyhow::Error>(result.unwrap().id))
    }
}

#[utoipa::path(
    post,
    path = "/api/users/{id}/verify",
    params( ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = "Users",
    responses(
        (status = 200, description = "Returns ok if the verification was successful."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the user couldn't be verified."),
    ),
    security(("JWT" = [])),
)]

#[post("/users/{id}/verify")]
async fn verify_user_handler(target_user: web::Path<uuid::Uuid>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    // Only admins may verify users
    
    let user_id = jwt.user_id;
    let target_id = target_user.into_inner();

    let check_admin = policy::require_admin(user_id, &state).await;

    if check_admin.is_some() {
        return check_admin.unwrap();
    }

    let result = user_db::verify_user(target_id, &state).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    get,
    path = "/api/users/{id}/info",
    params( ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = "Users",
    responses(
        (status = 200, description = "Returns the detailed user info.", body = UserInfo),
        (status = 401, description= "Returns an unauthorized error if no valid token was provided.<br>\
        Users may access their own info, other users can only be inspected by an admin."),
        (status = 500, description= "Returns an error if the user couldn't be retrieved."),
    ),
    security(("JWT" = [])),
)]

#[get("/users/{id}/info")]
async fn get_user_info_handler(target_user: web::Path<uuid::Uuid>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    // Each user can get its own info, only admins can get info of other users

    let user_id = jwt.user_id;
    let target_id = target_user.into_inner();

    let check = match user_id.is_some() && user_id.clone().unwrap() == target_id {
        true => policy::require_login(user_id, &state).await, // Same user - needs login
        false => policy::require_admin(user_id, &state).await // Different user - needs admin
    };

    if check.is_some() {
        return check.unwrap();
    }

    let result = cache::request_user(target_id, &state).await;
    
    main_hdl::send_result(&result.ok_or(anyhow::anyhow!("User with id {} not found!", target_id)))
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}/delete",
    params( ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = "Users",
    responses(
        (status = 200, description = "Returns ok on successful deletion of the user."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the user couldn't be deleted."),
    ),
    security(("JWT" = [])),
)]

#[delete("/users/{id}/delete")]
async fn delete_user_handler(target_user: web::Path<uuid::Uuid>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    // Each user may delete his own account, only admins can delete other users
    // Sensors owned by the deleted user become system sensors
    
    let user_id = jwt.user_id;
    let target_id = target_user.into_inner();
    
    let check = match user_id.is_some() && user_id.clone().unwrap() == target_id {
        true => policy::require_login(user_id, &state).await, // Same user - needs login
        false => { // Different user - needs admin
            let is_admin = policy::require_admin(user_id, &state).await;
            
            if is_admin.is_some() { // User is not admin
                is_admin
            } else if is_admin_user(target_id, &state).await { // Target user is also admin
                policy::unauthorized("Admin user can't be removed!".to_string())
            } else { // Ok
                None
            }
        }
    };

    if check.is_some() {
        return check.unwrap();
    }

    let result = user_db::delete_user(target_id, &state).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    post,
    path = "/api/users/{id}/role",
    params( ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string()))),
    request_body(
        content_type = "application/json",
        content = String,
        description = "The name of the role to be assigned to the user.",
    ),
    tag = "Users",
    responses(
        (status = 200, description = "Returns ok if the role was assigned successfully."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the role couldn't be assigned."),
    ),
    security(("JWT" = [])),
)]

#[post("/users/{id}/role")]
async fn assign_role_handler(target_user: web::Path<uuid::Uuid>, body: web::Json<String>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;

    let admin_check = policy::require_admin(user_id, &state).await;

    if admin_check.is_some() {
        return admin_check.unwrap();
    }
    
    let target_user_id = target_user.into_inner();
    let role_name = body.into_inner();

    let result = role_db::assign_role_by_name(target_user_id, role_name, false, &state).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}/role",
    params( ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string()))),
    request_body(
        content_type = "application/json",
        content = String,
        description = "The name of the role to be revoked from the user.",
    ),
    tag = "Users",
    responses(
        (status = 200, description = "Returns ok if the role was revoked successfully."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the role couldn't be revoked."),
    ),
    security(("JWT" = [])),
)]

#[delete("/users/{id}/role")]
async fn revoke_role_handler(target_user: web::Path<uuid::Uuid>, body: web::Json<String>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;

    let admin_check = policy::require_admin(user_id, &state).await;

    if admin_check.is_some() {
        return admin_check.unwrap();
    }

    let target_user_id = target_user.into_inner();
    let role_name = body.into_inner();

    let result = role_db::revoke_role(target_user_id, role_name, false, &state).await;

    main_hdl::send_result(&result)
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use actix_http::{Method, StatusCode};
    use super::*;
    use sqlx::PgPool;
    use serde_json::{json, Value};
    use sqlx::postgres::any::AnyConnectionBackend;
    use uuid::Uuid;
    use crate::database::models::api_key::ApiKey;
    use crate::database::models::role::{ROLE_SYSTEM_ADMIN, ROLE_SYSTEM_USER};
    use crate::database::user_db;
    use crate::database::models::user::UserInfo;
    use crate::features::cache;
    use crate::test_utils::tests::{anne, create_test_api_keys, create_test_app, create_test_sensors, execute_request, jane, john, login, test_invalid_auth};

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_list_users(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth("/api/users/list", Method::GET, None::<Value>, &state, &app).await;

        // --- List users as John - Should fail (not admin) ---

        let token = login(&john().email, &john().password, &app).await;

        let _ = execute_request("/api/users/list", Method::GET,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John admin and list users - Should succeed ---

        role_db::assign_role_by_name(john().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make John admin!");

        let body = execute_request("/api/users/list", Method::GET,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Check if all users and their roles are listed correctly

        let user_entries: Vec<UserInfo> = body.as_array()
            .unwrap_or(&vec![]).iter()
            .map(|entry| serde_json::from_value(entry.to_owned()).expect("Failed to parse user list entry"))
            .collect();

        for user in user_entries {
            if user.email == john().email {
                assert!(user.id == john().id && user.name == john().name);
                
                // Check roles
                let required_roles = vec!["system_test_role".to_string(), "test_role".to_string(), ROLE_SYSTEM_ADMIN.to_string()];
                let john_roles: Vec<String> = user.roles.iter().map(|r| r.name.clone()).collect();

                assert!(required_roles.iter().all(|item| john_roles.contains(item)));
                assert!(john_roles.iter().all(|item| required_roles.contains(item)));
            } else if user.email == anne().email {
                assert!(user.id == anne().id && user.name == anne().name);

                // Check roles
                let required_roles = vec!["system_test_role".to_string()];
                let ann_roles: Vec<String> = user.roles.iter().map(|r| r.name.clone()).collect();

                assert!(required_roles.iter().all(|item| ann_roles.contains(item)));
                assert!(ann_roles.iter().all(|item| required_roles.contains(item)));
            }
        }
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users"))]
    async fn test_register_user(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;
        
        // --- Register existing user - should not work ---

        let payload = json!({
            "name": "John Doe",
            "email": john().email,
            "password": john().password,
        });

        let _ = execute_request("/api/users/register", Method::POST,
                                   Some(payload), None,
                                   StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Register new user - should work  ---

        let payload = json!({
            "name": "Jim Dim",
            "email": "jim@gmail.com",
            "password": "MySecret",
        });

        let body = execute_request("/api/users/register", Method::POST,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // Check if user exists with correct values

        let user = user_db::get_user_by_id(Uuid::from_str(body.as_str().unwrap()).unwrap(), &state).await.unwrap();

        assert!(user.name.eq(&payload["name"]) && user.email.eq(&payload["email"]));
        
        // Check if user has one correct role - the user role

        let mut con = state.get_db_connection().await.unwrap();
        
        let roles = role_db::get_user_roles(user.id, con.as_mut()).await.unwrap();

        let _ = con.commit();
        
        assert!(roles.len() == 1 && roles.get(0).unwrap().name == ROLE_SYSTEM_USER);
        
        // --- Check if new user can log in - Should fail since not verified yet ---

        let payload = json!({
            "email": payload["email"],
            "password": payload["password"],
        });

        let _ = execute_request("/auth/login", Method::POST,
                                Some(payload), None,
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Register new admin user - should work over direct func ---

        let payload = json!({
            "name": "Jen Den",
            "email": "jen@gmail.com",
            "password": "MySecret"
        });
        
        let user_info: RegisterUserRequest = serde_json::from_value(payload).unwrap();
        
        let user = register_user(user_info, true, &state).await.unwrap();
        
        assert!(is_admin_user(user.id, &state).await);
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users"))]
    async fn test_verify_user(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth(format!("/api/users/{}/verify", jane().id).as_str(), Method::POST, None::<Value>, &state, &app).await;

        // --- Verify as John (no admin) - should fail ---

        let token = login(&john().email, &john().password, &app).await;
        
        let _ = execute_request(&format!("/api/users/{}/verify", jane().id), Method::POST,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John admin and verify Jane - should succeed

        role_db::assign_role_by_name(john().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/users/{}/verify", jane().id), Method::POST,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Verify already verified user - should succeed

        let _ = execute_request(&format!("/api/users/{}/verify", anne().id), Method::POST,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Verify non-existent user as admin - should fail

        let _ = execute_request(&format!("/api/users/{}/verify", Uuid::new_v4()), Method::POST,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_user_info(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth(format!("/api/users/{}/info", john().id).as_str(), Method::GET, None::<Value>, &state, &app).await;

        // --- Get info of John Doe (with his own login) - should succeed ---

        let token = login(&john().email, &john().password, &app).await;

        let body = execute_request(&format!("/api/users/{}/info", john().id), Method::GET,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Check if result has the correct data

        let user: UserInfo = serde_json::from_value(body).unwrap();

        assert!(user.id == john().id && user.name == john().name && user.email == john().email && user.verified == john().verified);

        // Check roles
        let required_roles = vec!["system_test_role".to_string(), "test_role".to_string()];
        let john_roles: Vec<String> = user.roles.iter().map(|r| r.name.clone()).collect();

        assert!(required_roles.iter().all(|item| john_roles.contains(item)));
        assert!(john_roles.iter().all(|item| required_roles.contains(item)));

        // --- List info of Anne Clark as user John Doe (not admin)  - should fail

        let _ = execute_request(&format!("/api/users/{}/info", anne().id), Method::GET,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John admin and get info of Anne Clark - should succeed

        role_db::assign_role_by_name(john().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/users/{}/info", anne().id), Method::GET,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Get info of non-existent user - should fail

        let _ = execute_request(&format!("/api/users/{}/info", Uuid::new_v4()), Method::GET,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_user(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let target_sensor_owned = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();

        test_invalid_auth(format!("/api/users/{}/delete", john().id).as_str(), Method::DELETE, None::<Value>, &state, &app).await;

        // --- Delete Anne as John - should fail ---

        let token = login(&john().email, &john().password, &app).await;

        let _ = execute_request(&format!("/api/users/{}/delete", anne().id), Method::DELETE,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John and Anne admin and delete Anne Clark - should fail

        role_db::assign_role_by_name(john().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make John admin!");
        role_db::assign_role_by_name(anne().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make Anne admin!");

        let _ = execute_request(&format!("/api/users/{}/delete", anne().id), Method::DELETE,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Revoke admin from Anne and delete her as John - Should succeed ---

        role_db::revoke_role(anne().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make Anne admin!");

        let _ = execute_request(&format!("/api/users/{}/delete", anne().id), Method::DELETE,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Delete non-existing user as john (admin) - should fail ---

        let _ = execute_request(&format!("/api/users/{}/delete", Uuid::new_v4()), Method::DELETE,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        
        // --- Delete John Doe (with his own login) - should succeed ---

        let _ = execute_request(&format!("/api/users/{}/delete", john().id), Method::DELETE,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Check if his sensor is now a system sensor

        let sensor = cache::request_sensor(target_sensor_owned.1, &state).await.unwrap();

        assert!(sensor.owner.is_none());

        // --- Delete John Doe again with his own login - should fail ---

        let _ = execute_request(&format!("/api/users/{}/delete", john().id), Method::DELETE,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::UNAUTHORIZED, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_assign_role(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth(format!("/api/users/{}/role", john().id).as_str(), Method::POST, Some(json!("test_role2")), &state, &app).await;

        // --- Try to assign a new role to John as John - Should fail (not admin) ---

        let token = login(&john().email, &john().password, &app).await;

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::POST,
                                Some(json!("test_role2")), Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make Anne admin and try to assign a new role to John - Should succeed ---
        
        role_db::assign_role_by_name(anne().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make Anne admin!");

        let token = login(&anne().email, &anne().password, &app).await;

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::POST,
                                Some(json!("test_role2")), Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Assign a role to John as Anne that does not exist - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::POST,
                                Some(json!("test_role2_non_exist")), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Assign a system role to John as Anne - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::POST,
                                Some(json!("system_test_role2")), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        
        // --- Assign a role to John as Anne that he already possess - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::POST,
                                Some(json!("test_role2")), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Assign a role to a non-existing user as Anne - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role", Uuid::new_v4()), Method::POST,
                                Some(json!("test_role2")), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_revoke_role(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;
        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor2").unwrap();

        test_invalid_auth(format!("/api/users/{}/role", john().id).as_str(), Method::DELETE, Some(json!("test_role2")), &state, &app).await;

        // --- Try to revoke a role from John as John - Should fail (not admin) ---

        let token = login(&john().email, &john().password, &app).await;

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::DELETE,
                                Some(json!("test_role")), Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make Anne admin and try to revoke a non-existing role from John - Should fail ---

        role_db::assign_role_by_name(anne().id, ROLE_SYSTEM_ADMIN.to_string(), true, &state).await.expect("Failed to make Anne admin!");

        let token = login(&anne().email, &anne().password, &app).await;

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::DELETE,
                                Some(json!("test_role_non_exist")), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Revoke an existing system role from John - Should fail ---

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::DELETE,
                                Some(json!("system_test_role")), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Force revoke system role from John and check if his api keys were removed ---
        
        role_db::revoke_role(john().id, "system_test_role".to_string(), true, &state).await.unwrap();
        
        let john_keys: Vec<ApiKey> = test_keys.iter().filter(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1).cloned().collect();

        assert_eq!(john_keys.len(), 2);

        for k in john_keys {
            assert!(cache::request_api_key(k.id, &state).await.is_none());
        }
        
        // --- Revoke an existing non-system role from John - Should succeed ---

        let _ = execute_request(&format!("/api/users/{}/role", john().id), Method::DELETE,
                                Some(json!("test_role")), Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Revoke an existing role from a non-existing user - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role", Uuid::new_v4()), Method::DELETE,
                                Some(json!("test_role")), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }
}