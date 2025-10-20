use super::models::requests::CreateRoleRequest;
use crate::authentication::jwt_auth;
use crate::database::models::role::Role;
use crate::database::role_db;
use crate::handler::{main_hdl, policy};
use crate::state::AppState;
use actix_web::{delete, get, post, web, Responder};

/* ------------------------------------------------ Roles -------------------------------------------------- */

const COMMON_TAG: &str = "Roles";

#[utoipa::path(
    get,
    path = "/api/roles/list",
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns a list of existing roles in the system.", body = Vec<Role>),
        (status = 401, description = "Returns an unauthorized error if no valid token was provided."),
    ),
    security(("JWT" = [])),
)]
#[get("/roles/list")]
async fn list_roles_handler(
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> impl Responder {
    // Every user may list all roles in the system (required for sensor permissions assignment)

    let user_id = jwt.user_id;

    let login_check = policy::require_login(user_id, &state).await;

    if login_check.is_some() {
        return login_check.unwrap();
    }

    let result = role_db::list_roles(&state).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    post,
    path = "/api/roles/create",
    request_body(
        content = CreateRoleRequest,
        description = "Name of the role to be created.",
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns the role on successful creation.", body = Role),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the role couldn't be created."),
    ),
    security(("JWT" = [])),
)]
#[post("/roles/create")]
async fn create_role_handler(
    body: web::Json<CreateRoleRequest>,
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> impl Responder {
    // Only admins may create new roles

    let user_id = jwt.user_id;

    let admin_check = policy::require_admin(user_id, &state).await;

    if admin_check.is_some() {
        return admin_check.unwrap();
    }

    let result = role_db::create_role(body.name.clone(), &state).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    delete,
    path = "/api/roles/{id}/delete",
    params( ("id" = String, Path, description = "The uuid of the role", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns ok on the successful deletion."),
        (status = 401, description= "Returns an unauthorized error if no valid admin token was provided."),
        (status = 500, description= "Returns an error if the role couldn't be deleted."),
    ),
    security(("JWT" = [])),
)]
#[delete("/roles/{id}/delete")]
async fn delete_role_handler(
    params: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> impl Responder {
    // Only admins may delete existing roles

    let user_id = jwt.user_id;

    let admin_check = policy::require_admin(user_id, &state).await;

    if admin_check.is_some() {
        return admin_check.unwrap();
    }

    let id = params.into_inner();

    let result = role_db::delete_role(id, false, &state).await;

    main_hdl::send_result(&result)
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::api_key::ApiKey;
    use crate::database::models::role::{
        ROLE_SYSTEM_ADMIN, ROLE_SYSTEM_GUEST, ROLE_SYSTEM_ROOT, ROLE_SYSTEM_USER,
    };
    use crate::features::cache;
    use crate::test_utils::tests::{
        anne, create_test_api_keys, create_test_app, create_test_sensors, execute_request, john,
        login, test_invalid_auth, TEST_ROLE, TEST_ROLE2, TEST_ROLE_THAT_NOT_EXISTS_BUT_IS_VALID,
        TEST_SYS_ROLE, TEST_SYS_ROLE2,
    };
    use actix_http::{Method, StatusCode};
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use uuid::Uuid;

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles"))]
    async fn test_list_roles(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth("/api/roles/list", Method::GET, None::<Value>, &state, &app).await;

        // -- List all roles as normal user - Should succeed --

        let token = login(&john(), &state).await;

        let body = execute_request(
            "/api/roles/list",
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;

        // Check if all roles are present

        let required_roles = vec![
            TEST_SYS_ROLE,
            TEST_ROLE,
            TEST_SYS_ROLE2,
            TEST_ROLE2,
            ROLE_SYSTEM_ADMIN,
            ROLE_SYSTEM_USER,
            ROLE_SYSTEM_GUEST,
            ROLE_SYSTEM_ROOT,
        ];

        let present_role_names: Vec<uuid::Uuid> = body
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|entry| {
                entry
                    .get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| Uuid::parse_str(&s.to_string()).unwrap())
                    .unwrap_or_default()
            })
            .collect();

        // Check in both directions if required and present are equal

        assert!(required_roles
            .iter()
            .all(|item| present_role_names.contains(item)));
        assert!(present_role_names
            .iter()
            .all(|item| required_roles.contains(item)));
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles"))]
    async fn test_create_role(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        test_invalid_auth(
            "/api/roles/create",
            Method::POST,
            Some(json!({"name":"new_role"})),
            &state,
            &app,
        )
        .await;

        // --- Create new role as normal user - Should fail ---

        let token = login(&john(), &state).await;

        let _ = execute_request(
            "/api/roles/create",
            Method::POST,
            None,
            Some(json!({"name":"new_role"})),
            Some(token.clone()),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Create new role as admin user - Should succeed ---

        role_db::assign_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state)
            .await
            .expect("Failed to make Anne admin!");

        let token = login(&anne(), &state).await;

        let _ = execute_request(
            "/api/roles/create",
            Method::POST,
            None,
            Some(json!({"name":"new_role"})),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;

        // --- Create duplicated role as admin user - Should fail ---

        let _ = execute_request(
            "/api/roles/create",
            Method::POST,
            None,
            Some(json!({"name":"new_role"})),
            Some(token.clone()),
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_role(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;
        let target_sensor_allowed = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor2")
            .unwrap();

        test_invalid_auth(
            format!("/api/roles/{}/delete", TEST_ROLE).as_str(),
            Method::DELETE,
            None::<Value>,
            &state,
            &app,
        )
        .await;

        // --- Delete existing non-system role as normal user - Should fail ---

        let token = login(&john(), &state).await;

        let _ = execute_request(
            format!("/api/roles/{}/delete", TEST_ROLE).as_str(),
            Method::DELETE,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Delete existing non-system role as admin user - Should succeed ---

        role_db::assign_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state)
            .await
            .expect("Failed to make Anne admin!");

        let token = login(&anne(), &state).await;

        let _ = execute_request(
            format!("/api/roles/{}/delete", TEST_ROLE).as_str(),
            Method::DELETE,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;

        // Check if john is no longer assigned to this role

        let john = cache::request_user(john().id, &state).await.unwrap();

        assert!(john.roles.iter().find(|r| r.id.eq(&TEST_ROLE)).is_none());

        // --- Delete non-existing role as admin user - Should fail ---

        let _ = execute_request(
            format!(
                "/api/roles/{}/delete",
                TEST_ROLE_THAT_NOT_EXISTS_BUT_IS_VALID
            )
            .as_str(),
            Method::DELETE,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // --- Delete existing system role as admin user - Should fail ---

        let _ = execute_request(
            format!("/api/roles/{}/delete", TEST_SYS_ROLE).as_str(),
            Method::DELETE,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::INTERNAL_SERVER_ERROR,
            &app,
        )
        .await;

        // --- Force delete system role and check if johns api keys were removed ---

        role_db::delete_role(TEST_SYS_ROLE, true, &state)
            .await
            .unwrap();

        let john_keys: Vec<ApiKey> = test_keys
            .iter()
            .filter(|k| k.user_id == tests::john().id && k.sensor_id == target_sensor_allowed.1)
            .cloned()
            .collect();

        assert_eq!(john_keys.len(), 2);

        for k in john_keys {
            assert!(cache::request_api_key(k.id, &state).await.is_none());
        }
    }
}
