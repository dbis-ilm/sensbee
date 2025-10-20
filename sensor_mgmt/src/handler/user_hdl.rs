use actix_web::{delete, get, post, web, Responder};
use crate::database::{role_db, user_db};
use crate::database::user_db::{is_admin_user, is_root_user, register_user};
use crate::handler::models::responses::GenericUuidResponse;
use crate::handler::{main_hdl, policy};
use crate::authentication::jwt_auth;
use crate::database::models::user::*;
use crate::features::cache;
use crate::handler::models::requests::{RegisterUserRequest,EditUserInfoRequest};
use crate::state::AppState;

/* ------------------------------------------------ User ------------------------------------------------------------ */

const COMMON_TAG: &str = "Users";

#[utoipa::path(
    get,
    path = "/api/users/list",
    tag = COMMON_TAG,
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
    tag = COMMON_TAG,
    responses(
        (status = 200, description= "Returns the uuid of the created user.", body = GenericUuidResponse),
        (status = 500, description= "Returns an error if the registration failed."),
    )
)]
#[post("/users/register")]
async fn register_user_handler(body: web::Json<RegisterUserRequest>, data: web::Data<AppState>) -> impl Responder {
    let result = register_user(body.into_inner(), &data).await;
    
    if result.is_err() {
        main_hdl::send_result(&result)
    } else { // Only send user_id back
        // TODO use generic uuid response here?
        main_hdl::send_result(&Ok::<uuid::Uuid, anyhow::Error>(result.unwrap().id))
    }
}

#[utoipa::path(
    post,
    path = "/api/users/{id}/edit/verify",
    params( ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns ok if the verification was successful."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the user couldn't be verified."),
    ),
    security(("JWT" = [])),
)]
#[post("/users/{id}/edit/verify")]
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
    tag = COMMON_TAG,
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
    post,
    path = "/api/users/{id}/edit/info",
    params( 
        ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string())),
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Information has been updated."),
        (status = 401, description= "Returns an unauthorized error if no valid token was provided.<br>\
        Users may edit their own info, Admins can edit any user."),
        (status = 500, description= "Returns an error if the user couldn't be edited."),
    ),
    security(("JWT" = [])),
)]
#[post("/users/{id}/edit/info")]
async fn edit_user_info_handler(target_user: web::Path<uuid::Uuid>, new_info: web::Json<EditUserInfoRequest>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;
    let target_id = target_user.into_inner();

    // Access Validation
    let check = match user_id.is_some() && user_id.clone().unwrap() == target_id {
        true => policy::require_login(user_id, &state).await, // Same user - needs login
        false => policy::require_admin(user_id, &state).await // Different user - needs admin
    };
    if check.is_some() {
        return check.unwrap();
    }

    let result = user_db::edit_user_info(target_id, new_info.into_inner(), &state).await;
    
    main_hdl::send_result(&result)
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}/delete",
    params( ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = COMMON_TAG,
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
            } else if is_root_user(target_id, &state).await { // If target is root -> we dont allow deletion
                policy::unauthorized("Root user cant be removed".to_string())
            } else if is_admin_user(target_id, &state).await {  // if target is admin -> check if requester is root
                if !is_root_user(user_id.unwrap(), &state).await {
                    policy::unauthorized("You must be root to remove an admin".to_string())
                } else {
                    None
                }
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
    path = "/api/users/{id}/role/{role_id}/assign",
    params( 
        ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string())),
        ("role_id" = String, Path, description = "The id of the role to assign", example = json!(uuid::Uuid::new_v4().to_string())),
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns ok if the role was assigned successfully."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the role couldn't be assigned."),
    ),
    security(("JWT" = [])),
)]
#[post("/users/{id}/role/{role_id}/assign")]
async fn assign_role_handler(params: web::Path<(uuid::Uuid, uuid::Uuid)>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;

    let admin_check = policy::require_admin(user_id, &state).await;

    if admin_check.is_some() {
        return admin_check.unwrap();
    }

    let params = params.into_inner();
    let target_user_id = params.0;
    let role_id = params.1;

    // This endpoint is not allowed to assign system roles besieds the admin role
    match cache::request_role(role_id, &state).await {
        Some(role) => {
            if role.system && !role.is_admin(){
                return policy::unauthorized("only admin system role can be assigned".to_string()).unwrap();
            }
        },
        None => (),
    };

    let result = role_db::assign_role(target_user_id, role_id, is_root_user(user_id.unwrap(), &state).await, &state).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}/role/{role_id}/revoke",
    params( 
        ("id" = String, Path, description = "The uuid of the user", example = json!(uuid::Uuid::new_v4().to_string())),
        ("role_id" = String, Path, description = "The id of the role to remove"),
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns ok if the role was revoked successfully."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the role couldn't be revoked."),
    ),
    security(("JWT" = [])),
)]

#[delete("/users/{id}/role/{role_id}/revoke")]
async fn revoke_role_handler(params: web::Path<(uuid::Uuid, uuid::Uuid)>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;

    let admin_check = policy::require_admin(user_id, &state).await;

    if admin_check.is_some() {
        return admin_check.unwrap();
    }

    let params = params.into_inner();
    let target_user_id = params.0;
    let role_id = params.1;

    // This endpoint is not allowed to revoke system roles besieds the admin role
     match cache::request_role(role_id, &state).await {
        Some(role) => {
            if role.system && !role.is_admin(){
                return policy::unauthorized("only admin system role can be revoked".to_string()).unwrap();
            }
        },
        None => (),
    };

    let result = role_db::revoke_role(target_user_id, role_id, is_root_user(user_id.unwrap(), &state).await, &state).await;

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
    use uuid::Uuid;
    use crate::database::models::api_key::ApiKey;
    use crate::database::models::role::{ROLE_SYSTEM_ADMIN, ROLE_SYSTEM_GUEST, ROLE_SYSTEM_ROOT, ROLE_SYSTEM_USER};
    use crate::database::user_db;
    use crate::database::models::user::UserInfo;
    use crate::features::cache;
    use crate::handler::models::requests::EditUserInfoRequest;
    use crate::test_utils::tests::{anne, create_test_api_keys, create_test_app, create_test_sensors, execute_request, jack, jane, john, login, test_invalid_auth, TEST_ROLE, TEST_ROLE2, TEST_ROLE_THAT_NOT_EXISTS_BUT_IS_VALID, TEST_SYS_ROLE, TEST_SYS_ROLE2};

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_list_users(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth("/api/users/list", Method::GET, None::<Value>, &state, &app).await;

        // --- List users as John - Should fail (not admin) ---

        let token = login(&john(), &state).await;

        let _ = execute_request("/api/users/list", Method::GET, None,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John admin and list users - Should succeed ---

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let body = execute_request("/api/users/list", Method::GET, None,
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
                let required_roles = vec![TEST_SYS_ROLE, TEST_ROLE, ROLE_SYSTEM_ADMIN];
                let john_roles: Vec<Uuid> = user.roles.iter().map(|r| r.id.clone()).collect();

                assert!(required_roles.iter().all(|item| john_roles.contains(item)));
                assert!(john_roles.iter().all(|item| required_roles.contains(item)));
            } else if user.email == anne().email {
                assert!(user.id == anne().id && user.name == anne().name);

                // Check roles
                let required_roles = vec![TEST_SYS_ROLE];
                let ann_roles: Vec<Uuid> = user.roles.iter().map(|r| r.id.clone()).collect();

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
        });

        let _ = execute_request("/api/users/register", Method::POST, None,
                                   Some(payload), None,
                                   StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Register new user - should work  ---

        let payload = json!({
            "name": "Jim Dim",
            "email": "jim@gmail.com",
        });

        let body = execute_request("/api/users/register", Method::POST, None,
                                Some(payload.clone()), None,
                                StatusCode::OK, &app).await;

        // Check if user exists with correct values

        let user = user_db::get_user_by_id(Uuid::from_str(body.as_str().unwrap()).unwrap(), &state).await.unwrap();

        assert!(user.name.eq(&payload["name"]) && user.email.eq(&payload["email"]));
        
        // Check if user has one correct role - the user role

        let mut con = state.db.begin().await.unwrap();
        
        let roles = role_db::get_user_roles(user.id, con.as_mut()).await.unwrap();

        let _ = con.commit().await;
        
        assert!(roles.len() == 1 && roles.get(0).unwrap().id == ROLE_SYSTEM_USER);
        
        // --- Check if new user can log in - Should fail since not verified yet ---

        let payload = json!({
            "email": payload["email"],
        });

        let _ = execute_request("/auth/dev/login", Method::POST, None,
                                Some(payload), None,
                                StatusCode::UNAUTHORIZED, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users"))]
    async fn test_verify_user(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth(format!("/api/users/{}/edit/verify", jane().id).as_str(), Method::POST, None::<Value>, &state, &app).await;

        // --- Verify as John (no admin) - should fail ---

        let token = login(&john(), &state).await;
        
        let _ = execute_request(&format!("/api/users/{}/edit/verify", jane().id), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John admin and verify Jane - should succeed

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/users/{}/edit/verify", jane().id), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Verify already verified user - should succeed

        let _ = execute_request(&format!("/api/users/{}/edit/verify", anne().id), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Verify non-existent user as admin - should fail

        let _ = execute_request(&format!("/api/users/{}/edit/verify", Uuid::new_v4()), Method::POST, None,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_user_info(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth(format!("/api/users/{}/info", john().id).as_str(), Method::GET, None::<Value>, &state, &app).await;

        // --- Get info of John Doe (with his own login) - should succeed ---

        let token = login(&john(), &state).await;

        let body = execute_request(&format!("/api/users/{}/info", john().id), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Check if result has the correct data

        let user: UserInfo = serde_json::from_value(body).unwrap();

        assert!(user.id == john().id && user.name == john().name && user.email == john().email && user.verified == john().verified);

        // Check roles
        let required_roles = vec![TEST_SYS_ROLE, TEST_ROLE];
        let john_roles: Vec<Uuid> = user.roles.iter().map(|r| r.id.clone()).collect();

        assert!(required_roles.iter().all(|item| john_roles.contains(item)));
        assert!(john_roles.iter().all(|item| required_roles.contains(item)));

        // --- List info of Anne Clark as user John Doe (not admin)  - should fail

        let _ = execute_request(&format!("/api/users/{}/info", anne().id), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John admin and get info of Anne Clark - should succeed

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/users/{}/info", anne().id), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Get info of non-existent user - should fail

        let _ = execute_request(&format!("/api/users/{}/info", Uuid::new_v4()), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_edit_user(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;
        
        // --- make sure that unauthorized calls get a 401

        // NOTE Without these params we get a 400 instead of the expected 401
        let some_payload = json!({
            "name": &john().name,
            "email": &john().email,
        });
        let payload: EditUserInfoRequest = serde_json::from_value(some_payload.clone()).unwrap();
        test_invalid_auth(format!("/api/users/{}/edit/info", john().id).as_str(), Method::POST, Some(payload), &state, &app).await;

        // --- base case - let a user edit their own information - should work

        let token = login(&john(), &state).await;
        let body = execute_request(&format!("/api/users/{}/info", john().id), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
        let base_user: UserInfo = serde_json::from_value(body).unwrap();

        // create updated info
        let payload = json!({
            "name": "John Doe-Dove",
            "email": format!("other_{:?}",john().email),
        });
        let proposed_user_info_changes: EditUserInfoRequest = serde_json::from_value(payload.clone()).unwrap();
    
        let _ = execute_request(&format!("/api/users/{}/edit/info", base_user.id), Method::POST, None,
                                Some(proposed_user_info_changes.clone()), Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        // Validate that the values have been updated
        let updated_user_info = execute_request(&format!("/api/users/{}/info", john().id), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        assert_eq!(updated_user_info["name"], payload["name"]);
        assert_eq!(updated_user_info["email"], payload["email"]);

        // --- a non admin user tries to edit another user -- should fail

        let _ = execute_request(&format!("/api/users/{}/edit/info", &jane().id), Method::POST, None,
                                Some(proposed_user_info_changes), Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- empty but valid payload -- should fail

        let nearly_empty_payload = json!({
            "name": "",
            "email": "",
        });
        let proposed_user_info_changes: EditUserInfoRequest = serde_json::from_value(nearly_empty_payload.clone()).unwrap();
    
        let _ = execute_request(&format!("/api/users/{}/edit/info", base_user.id), Method::POST, None,
                                Some(proposed_user_info_changes.clone()), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- an admin should be able to change the information of any other user -- should work

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let admin_req_payload = json!({
            "name": "A name",
            "email": "a@email",
        });
        let proposed_user_info_changes_by_admin: EditUserInfoRequest = serde_json::from_value(admin_req_payload.clone()).unwrap();

        let _ = execute_request(&format!("/api/users/{}/edit/info", jane().id), Method::POST, None,
                                Some(proposed_user_info_changes_by_admin.clone()), Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Validate that the values have been updated
        let updated_user_info = execute_request(&format!("/api/users/{}/info", jane().id), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        assert_eq!(updated_user_info["name"], admin_req_payload["name"]);
        assert_eq!(updated_user_info["email"], admin_req_payload["email"]);

        // --- edit a non-existing user -- should fail

        let _ = execute_request(&format!("/api/users/{}/edit/info", Uuid::new_v4()), Method::POST, None,
                                Some(proposed_user_info_changes_by_admin.clone()), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_user(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let target_sensor_owned = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();

        test_invalid_auth(format!("/api/users/{}/delete", john().id).as_str(), Method::DELETE, None::<Value>, &state, &app).await;

        // --- Delete Anne as John while both only have user roles - should fail ---

        let token = login(&john(), &state).await;

        let _ = execute_request(&format!("/api/users/{}/delete", anne().id), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make John and Anne admin and delete Anne Clark - should fail

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");
        role_db::assign_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make Anne admin!");

        let _ = execute_request(&format!("/api/users/{}/delete", anne().id), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Revoke admin from Anne and delete her as John - Should succeed ---

        role_db::revoke_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make Anne admin!");

        let _ = execute_request(&format!("/api/users/{}/delete", anne().id), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Delete non-existing user as john (admin) - should fail ---

        let _ = execute_request(&format!("/api/users/{}/delete", Uuid::new_v4()), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        
        // --- Assign John and Jack the root role and delete John as Jack - should fail ---

        role_db::assign_role(john().id, ROLE_SYSTEM_ROOT, true, &state).await.expect("Failed to make John root!");
        role_db::assign_role(jack().id, ROLE_SYSTEM_ROOT, true, &state).await.expect("Failed to make Jack root!");

        let _ = execute_request(&format!("/api/users/{}/delete", jack().id), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        
        // --- Revoke root role and assign admin then try delete again - shoud work ---

        role_db::revoke_role(jack().id, ROLE_SYSTEM_ROOT, true, &state).await.expect("Failed to revoke root from Jack!");
        role_db::assign_role(jack().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make Jack admin!");

        let _ = execute_request(&format!("/api/users/{}/delete", jack().id), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        // --- Delete John Doe (with his own login) - should succeed ---

        let _ = execute_request(&format!("/api/users/{}/delete", john().id), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Check if his sensor is now a system sensor

        let sensor = cache::request_sensor(target_sensor_owned.1, &state).await.unwrap();

        assert!(sensor.owner.is_none());

        // --- Delete John Doe again with his own login - should fail ---

        let _ = execute_request(&format!("/api/users/{}/delete", john().id), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_assign_role(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth(format!("/api/users/{}/role/{}/assign", john().id,TEST_ROLE2).as_str(), Method::POST, None::<Value>, &state, &app).await;

        // --- Try to assign a new role to John as John - Should fail (not admin) ---

        let token = login(&john(), &state).await;

        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, TEST_ROLE2), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        
        // --- Assign any default system role as John - should fail ---
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_ROOT), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_ADMIN), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_USER), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_GUEST), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make Anne admin and try to assign a new role to John - Should succeed ---
        
        role_db::assign_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make Anne admin!");

        let token = login(&anne(), &state).await;

        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, TEST_ROLE2), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Assign a role to John as Anne that does not exist - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, TEST_ROLE_THAT_NOT_EXISTS_BUT_IS_VALID), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Assign a system role to John as Anne - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, TEST_SYS_ROLE2), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        
        // --- Assign any default system role as Anne - should fail ---
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_ROOT), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        // internal error as this is a double assignment
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_ADMIN), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_USER), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_GUEST), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        
        // --- Assign a role to John as Anne that he already possess - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, TEST_ROLE2), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Assign a role to a non-existing user as Anne - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", Uuid::new_v4(), TEST_ROLE2), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Assign root role to anne and make john Admin -- shoud work ---
        role_db::assign_role(anne().id, ROLE_SYSTEM_ROOT, true, &state).await.expect("Failed to make Anne admin!");
        
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_ADMIN), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        // --- Assign any other default system role as Anne - should fail ---
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_ROOT), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_USER), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/assign", john().id, ROLE_SYSTEM_GUEST), Method::POST, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_revoke_role(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;
        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor2").unwrap();

        test_invalid_auth(format!("/api/users/{}/role/{}/revoke", john().id, TEST_ROLE2).as_str(), Method::DELETE, None::<Value>, &state, &app).await;

        // --- Try to revoke a role from John as John - Should fail (not admin) ---

        let token = login(&john(), &state).await;

        // --- Revoke the test role as non admin user john - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, TEST_ROLE), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Revoke any default system role as John - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_ROOT), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_ADMIN), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_USER), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_GUEST), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Make Anne admin and try to revoke a non-existing role from John - Should fail ---

        role_db::assign_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make Anne admin!");

        let token = login(&anne(), &state).await;

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, TEST_ROLE_THAT_NOT_EXISTS_BUT_IS_VALID), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Revoke an existing system role from John - Should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, TEST_SYS_ROLE), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Revoke any default system role as anne with admin role - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", anne().id, ROLE_SYSTEM_ROOT), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", anne().id, ROLE_SYSTEM_ADMIN), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", anne().id, ROLE_SYSTEM_USER), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", anne().id, ROLE_SYSTEM_GUEST), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Force revoke system role from John and check if his api keys were removed ---
        
        role_db::revoke_role(john().id, TEST_SYS_ROLE, true, &state).await.unwrap();
        
        let john_keys: Vec<ApiKey> = test_keys.iter().filter(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1).cloned().collect();

        assert_eq!(john_keys.len(), 2);

        for k in john_keys {
            assert!(cache::request_api_key(k.id, &state).await.is_none());
        }
        
        // --- Revoke an existing non-system role from John - Should succeed ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, TEST_ROLE), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Revoke an existing role from a non-existing user - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", Uuid::new_v4(), TEST_ROLE), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        
        // Assign root role and revoke admin - shoud work ---

        role_db::assign_role(anne().id, ROLE_SYSTEM_ROOT, true, &state).await.expect("Failed to make Anne admin!");

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_ADMIN), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Revoke any other system role - should fail ---

        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_ROOT), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_USER), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        let _ = execute_request(&format!("/api/users/{}/role/{}/revoke", john().id, ROLE_SYSTEM_GUEST), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

    }
}