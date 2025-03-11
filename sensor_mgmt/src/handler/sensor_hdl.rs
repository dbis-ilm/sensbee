use actix_web::{delete, get, post, web, Responder};
use serde_json::{json,Value};
use crate::database::models::db_structs::DBOperation;
use crate::database::models::sensor::ShortSensorInfo;
use crate::database::sensor_db::{create_sensor, delete_sensor, edit_sensor, get_sensor_overview};
use crate::handler::{main_hdl, policy};
use crate::authentication::jwt_auth;
use crate::database::models::api_key::ApiKey;
use crate::database::sensor_db;
use crate::features::cache;
use crate::handler::policy::unauthorized;
use crate::handler::models::requests::{CreateApiKeyRequest, CreateSensorRequest, EditSensorRequest};
use crate::handler::models::responses::{GenericUuidResponse, SensorDetailResponse};
use crate::features::user_sens_perm::UserSensorPerm;
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/api/sensors/list",
    tag = "Sensors",
    responses(
        (status = 200, description= "Return list of accessible sensors.", body = Vec<ShortSensorInfo>),
    )
)]

#[get("/sensors/list")]
async fn list_sensors_handler(data: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;

    let login_check = policy::require_login(user_id, &data).await;
    let login_id = login_check.map_or(user_id, |_| None);

    // Fetch all sensor overviews

    let result = get_sensor_overview(&data).await;
    
    if result.is_err() {
        return main_hdl::send_result(&result);
    }
    
    // Filter sensors that the user or guest can access (INFO)
    // Might be quite expensive for many sensors in the system!
    
    let all_sens = result.unwrap();

    let mut filtered: Vec<ShortSensorInfo> = Vec::new();

    for sensor in all_sens {
        let full_sensor = cache::request_sensor(sensor.id, &data).await;
        
        if full_sensor.is_none() { 
            continue;
        }
        
        let perm = sensor_db::get_user_sensor_permissions(login_id, &full_sensor.unwrap(), &data).await;
        
        if perm.has(UserSensorPerm::Info) {
            filtered.push(sensor);
        }
    }
    
    let res: Result<Vec<ShortSensorInfo>, anyhow::Error> = Ok(filtered);
    
    main_hdl::send_result(&res)
}

#[utoipa::path(
    get,
    path = "/api/sensors/{id}/info",
    tag = "Sensors",
    params( ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string()))),
    responses(
        (status = 200, description= "Detailed information about the specified sensor.", body = SensorDetailResponse),
        (status = 401, description= "Returns an unauthorized error if access is not permitted (no token or no guest access)."),
        (status = 500, description= "Returns an error if the sensor does not exist."),
    )
)]

#[get("/sensors/{id}/info")]
async fn get_sensor_info_handler(path: web::Path<uuid::Uuid>, data: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;
    let sensor_id = path.into_inner();
    
    // First checks, if sensor exists

    let sensor = cache::request_sensor(sensor_id, &data).await;

    let res = match sensor {
        Some(sensor) => Ok(sensor),
        None => Err(anyhow::anyhow!("Sensor with id {} not found!", sensor_id))
    };
    
    if res.is_err() {
        return main_hdl::send_result(&res);
    }
    
    let mut sensor = res.unwrap();

    // Extracts user id if login is valid or None (Guest)

    let login_id = policy::require_login(user_id, &data).await.map_or(user_id, |_| None);

    // Retrieve permission set of user for sensor

    let permissions = sensor_db::get_user_sensor_permissions(login_id, &sensor, &data).await;
    
    // Check if user is allowed to get sensor info
    
    if !permissions.has(UserSensorPerm::Info) {
        return policy::unauthorized("No permissions to get sensor info!".to_string()).unwrap();
    }

    // Purge sensitive data from sensor for users without edit permission

    if !permissions.has(UserSensorPerm::Edit) {
        sensor.permissions.clear()
    }

    // Retrieve api keys for user and sensor
    
    let api_keys = match user_id {
        Some(id) => {
            let api_res = sensor_db::get_api_keys(sensor_id, id, &data).await;

            if api_res.is_err() {
                return main_hdl::send_result(&api_res);
            }
            
            api_res.unwrap()
        },
        None => Vec::default()
    };
    
    let res = SensorDetailResponse {
        sensor_info: sensor,
        user_permissions: permissions,
        api_keys,
    };

    main_hdl::send_result(&Ok(res))
}

#[utoipa::path(
    post,
    path = "/api/sensors/create",
    request_body(
        content_type = "application/json",
        content = CreateSensorRequest,
        description = "Description of the sensor.",
        example = json!({"name":"MySensor","description":"This is my first sensor.","position":[50.68322,10.91858],"permissions":[{"role_name":"user","operations":["INFO","READ","WRITE"]}],"columns":[{"name":"count","val_type":"INT","val_unit":"number"},{"name":"temperature","val_type":"FLOAT","val_unit":"celsius"}], "storage": {"variant": "DEFAULT", "params": {}}}),
    ),
    tag = "Sensors",
    responses(
        (status = 200, description = "Sensor id (uuid) of the newly registered sensor.", body = GenericUuidResponse),
        (status = 401, description= "Returns an unauthorized error if no valid token was provided."),
    ),
    security(("JWT" = [])),
)]

#[post("/sensors/create")]
async fn create_sensor_handler(body: web::Json<CreateSensorRequest>, data: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;
    
    let login_check = policy::require_login(user_id, &data).await;
    
    if login_check.is_some() {
        return login_check.unwrap();
    }

    let result = create_sensor(body.into_inner(), user_id, &data).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    post,
    path = "/api/sensors/{id}/edit",
    request_body(
        content_type = "application/json",
        content = EditSensorRequest,
        description = "Description of the sensor.",
        example = json!({"name":"MySensor","description":"This is my first sensor.","position":[50.68322,10.91858],"permissions":[{"role_name":"user","operations":["INFO","READ"]}], "storage": {"variant": "DEFAULT", "params": {}}}),
    ),
    params( ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = "Sensors",
    responses(
        (status = 200, description = "Returns ok if the edit was successful."),
        (status = 401, description= "Returns an unauthorized error if no valid admin or owner token was provided."),
        (status = 500, description= "Returns an error if the sensor does not exist or couldn't be edited."),
    ),
    security(("JWT" = [])),
)]

#[post("/sensors/{id}/edit")]
async fn edit_sensor_handler(path: web::Path<uuid::Uuid>, body: web::Json<EditSensorRequest>, data: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;
    let sensor_id = path.into_inner();

    let perm_check = policy::require_sensor_permission(user_id, sensor_id, UserSensorPerm::Edit, &data).await;
    
    if perm_check.is_some() {
        return perm_check.unwrap();
    }

    let result = edit_sensor(sensor_id, body.into_inner(), &data).await;

    main_hdl::send_result(&result)
}

#[utoipa::path(
    delete,
    path = "/api/sensors/{id}/delete",
    params( ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = "Sensors",
    responses(
        (status = 200, description = "Returns ok if the deletion was successful."),
        (status = 401, description= "Returns an unauthorized error if no valid admin or owner token was provided."),
        (status = 500, description= "Returns an error if the sensor does not exist or couldn't be deleted."),
    ),
    security(("JWT" = [])),
)]

#[delete("/sensors/{id}/delete")]
async fn delete_sensor_handler(path: web::Path<uuid::Uuid>, data: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;
    let sensor_id = path.into_inner();

    let perm_check = policy::require_sensor_permission(user_id, sensor_id, UserSensorPerm::Delete,  &data).await;

    if perm_check.is_some() {
        return perm_check.unwrap();
    }
    
    let result = delete_sensor(sensor_id, &data).await;
    
    main_hdl::send_result(&result)
}

/* ---------------------------------------------------API Keys ---------------------------------------------------------------- */

#[utoipa::path(
    post,
    path = "/api/sensors/{id}/api_key/create",
    params( ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string()))),
    request_body(
        content_type = "application/json",
        content = CreateApiKeyRequest,
        description = "Description of the api key.",
        example = json!({"name":"MyKey","operation":DBOperation::READ}),
    ),
    tag = "Sensors",
    responses(
        (status = 200, description = "Returns ok if the api key was created.", body = ApiKey),
        (status = 401, description= "Returns an unauthorized error if no permissions to create an api key."),
        (status = 500, description= "Returns an error if the api key couldn't be created."),
    ),
    security(("JWT" = [])),
)]

#[post("/sensors/{id}/api_key/create")]
async fn create_sensor_api_key_handler(path: web::Path<uuid::Uuid>, body: web::Json<CreateApiKeyRequest>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;
    let sensor_id = path.into_inner();

    let perm_check = match body.operation {
        DBOperation::READ => policy::require_sensor_permission(user_id, sensor_id, UserSensorPerm::ApiKeyRead, &state).await,
        DBOperation::WRITE => policy::require_sensor_permission(user_id, sensor_id, UserSensorPerm::ApiKeyWrite, &state).await,
        _ => unauthorized("Invalid api key operation!".to_string())
    };

    if perm_check.is_some() {
        return perm_check.unwrap();
    }

    let key_res = sensor_db::create_api_key(sensor_id, user_id.unwrap(), body.into_inner(), &state).await;

    main_hdl::send_result(&key_res)
}

#[utoipa::path(
    delete,
    path = "/api/sensors/{id}/api_key/{key_id}/delete",
    params( 
        ("id" = String, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string())),
        ("key_id" = String, Path, description = "The uuid of the key", example = json!(uuid::Uuid::new_v4().to_string()))
    ),
    tag = "Sensors",
    responses(
        (status = 200, description = "Returns ok if the api key was deleted."),
        (status = 401, description= "Returns an unauthorized error if no permissions to delete the api key."),
        (status = 500, description= "Returns an error if the api key couldn't be deleted."),
    ),
    security(("JWT" = [])),
)]

#[delete("/sensors/{id}/api_key/{key_id}/delete")]
async fn delete_sensor_api_key_handler(path: web::Path<(uuid::Uuid, uuid::Uuid)>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    let user_id = jwt.user_id;
    
    let params = path.into_inner();
    
    let sensor_id = params.0;
    let key_id = params.1;
    
    let key = cache::request_api_key(key_id, &state).await;
    
    if key.is_none() {
        return unauthorized("Invalid api key!".to_string()).unwrap();
    }
    
    let key = key.unwrap();

    let perm_check = match key.operation {
        DBOperation::READ => policy::require_sensor_permission(user_id, sensor_id, UserSensorPerm::ApiKeyRead, &state).await,
        DBOperation::WRITE => policy::require_sensor_permission(user_id, sensor_id, UserSensorPerm::ApiKeyWrite, &state).await,
        _ => unauthorized("Invalid api key operation!".to_string())
    };

    if perm_check.is_some() {
        return perm_check.unwrap();
    }

    let mut con = state.db.begin().await.unwrap();

    let res = sensor_db::delete_api_keys(vec![key.id], con.as_mut(), &state).await;

    let _ = con.commit().await;

    main_hdl::send_result(&res)
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use actix_http::Method;
    use actix_web::http::StatusCode;
    use super::*;
    use sqlx::PgPool;
    use uuid::Uuid;
    use crate::database::models::role::{ROLE_SYSTEM_ADMIN, ROLE_SYSTEM_USER};
    use crate::database::models::sensor::{ColumnType, SensorColumn};
    use crate::database::role_db;
    use crate::features::sensor_data_storage::{SensorDataStorageCfg, SensorDataStorageType};
    use crate::handler::models::requests::SensorPermissionRequest;
    use crate::test_utils::tests::{anne, create_test_api_keys, create_test_app, create_test_sensors, execute_request, john, login, test_invalid_auth, TEST_SYS_ROLE};

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_list_sensors(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        create_test_sensors(&state).await;

        // --- List public sensors without login - should succeed ---

        let body = execute_request("/api/sensors/list", Method::GET, None,
                                   None::<Value>, None,
                                   StatusCode::OK, &app).await;

        // Check, if we see the only public sensor
        let required_sensors = vec!["MySensor5".to_string()];
        let present_sensors: Vec<String> = body.as_array()
            .unwrap_or(&vec![]).iter()
            .map(|entry| serde_json::from_value(entry.to_owned()).expect("Failed to parse sensor info!"))
            .map(|info: ShortSensorInfo | info.name)
            .collect();

        assert!(required_sensors.iter().all(|item| present_sensors.contains(item)));
        assert!(present_sensors.iter().all(|item| required_sensors.contains(item)));

        // --- List sensors of John Doe with his login - should succeed ---

        let token = login(&john().email, &john().password, &app).await;

        let body = execute_request("/api/sensors/list", Method::GET, None,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::OK, &app).await;

        // Check if john sees the correct sensors

        let required_sensors = vec!["MySensor".to_string(), "MySensor2".to_string(), "MySensor3".to_string(), "MySensor5".to_string()];
        let present_sensors: Vec<String> = body.as_array()
            .unwrap_or(&vec![]).iter()
            .map(|entry| serde_json::from_value(entry.to_owned()).expect("Failed to parse sensor info!"))
            .map(|info: ShortSensorInfo | info.name)
            .collect();

        // Check in both directions if required and present are equal

        assert!(required_sensors.iter().all(|item| present_sensors.contains(item)));
        assert!(present_sensors.iter().all(|item| required_sensors.contains(item)));

        // --- List sensors of Anne (admin) with her login - should succeed ---

        role_db::assign_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make Anne admin!");

        let token = login(&anne().email, &anne().password, &app).await;

        let body = execute_request("/api/sensors/list", Method::GET, None,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::OK, &app).await;

        // Check if anne can see all sensors (is admin)

        let required_sensors = vec!["MySensor".to_string(), "MySensor2".to_string(), "MySensor3".to_string(),
                                    "MySensor4".to_string(), "MySensor5".to_string()];
        let present_sensors: Vec<String> = body.as_array()
            .unwrap_or(&vec![]).iter()
            .map(|entry| serde_json::from_value(entry.to_owned()).expect("Failed to parse sensor info!"))
            .map(|info: ShortSensorInfo | info.name)
            .collect();

        // Check in both directions if required and present are equal

        assert!(required_sensors.iter().all(|item| present_sensors.contains(item)));
        assert!(present_sensors.iter().all(|item| required_sensors.contains(item)));
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_get_sensor_info(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        create_test_api_keys(&state).await;

        let target_sensor_own = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();
        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor2").unwrap();
        let target_sensor_not_allowed = test_sens.iter().find(|(name, _)| name == "MySensor4").unwrap();
        let public_sensor = test_sens.iter().find(|(name, _)| name == "MySensor5").unwrap();

        test_invalid_auth(format!("/api/sensors/{}/info", &target_sensor_own.1).as_str(), Method::GET, None::<Value>, &state, &app).await;

        // --- Access own sensor as John - should succeed ---

        let token = login(&john().email, &john().password, &app).await;

        let body = execute_request(&format!("/api/sensors/{}/info", &target_sensor_own.1), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Check if correct data is returned

        let sensor: SensorDetailResponse = serde_json::from_value(body).unwrap();

        assert_eq!(sensor.sensor_info.name, "MySensor");
        assert_eq!(sensor.sensor_info.owner, Some(john().id));
        assert!(sensor.user_permissions.has_all());
        assert_eq!(sensor.api_keys.len(), 2); // John has one read and one write api key
        assert_eq!(sensor.sensor_info.storage_type, SensorDataStorageType::Default);
        assert_eq!(sensor.sensor_info.storage_params, None);

        for key in sensor.api_keys.iter() { // Validate both keys
            assert!((key.operation == DBOperation::READ && key.name == "TestKeyRead" ||
                key.operation == DBOperation::WRITE && key.name == "TestKeyWrite") &&
            key.sensor_id == sensor.sensor_info.id && key.user_id == john().id);
        }

        // --- Access allowed sensor as John - should succeed ---

        let body = execute_request(&format!("/api/sensors/{}/info", &target_sensor_allowed.1), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        let sensor: SensorDetailResponse = serde_json::from_value(body).unwrap();

        // John only as Info, Read, Write permissions
        assert!(sensor.user_permissions.has(UserSensorPerm::Info) && sensor.user_permissions.has(UserSensorPerm::Read)
            && sensor.user_permissions.has(UserSensorPerm::Write) && sensor.user_permissions.has(UserSensorPerm::ApiKeyRead)
            && sensor.user_permissions.has(UserSensorPerm::ApiKeyWrite) && !sensor.user_permissions.has(UserSensorPerm::Edit)
            && !sensor.user_permissions.has(UserSensorPerm::Delete));

        // --- Access not-allowed sensor as John - should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/info", &target_sensor_not_allowed.1), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Access not-allowed sensor as John (admin) - should succeed ---

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/sensors/{}/info", &target_sensor_not_allowed.1), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Access non-existing sensor as John (admin) - should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/info", Uuid::new_v4()), Method::GET, None,
                                   None::<Value>, Some(token.clone()),
                                   StatusCode::INTERNAL_SERVER_ERROR, &app).await;

        // --- Access public sensor as John (admin) - should succeed ---

        let _ = execute_request(&format!("/api/sensors/{}/info", &public_sensor.1), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Access public sensor without login - should succeed ---

        let body = execute_request(&format!("/api/sensors/{}/info", &public_sensor.1), Method::GET, None,
                                None::<Value>, None,
                                StatusCode::OK, &app).await;

        let sensor: SensorDetailResponse = serde_json::from_value(body).unwrap();

        // Guests can't view sensor permissions
        assert!(sensor.sensor_info.permissions.is_empty());
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_create_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        fn create_request(name: String) -> CreateSensorRequest {
            CreateSensorRequest {
                name: name,
                description: Some("My new sensor!".to_string()),
                position: Some((50.0, 10.0)),
                permissions: vec![SensorPermissionRequest { role_id: ROLE_SYSTEM_USER, operations: vec![DBOperation::INFO, DBOperation::READ] }],
                columns: vec![
                    SensorColumn {
                        name: "col1".to_string(), val_type: ColumnType::INT, val_unit: "unit_1".to_string(),
                    },
                    SensorColumn {
                        name: "col2".to_string(), val_type: ColumnType::FLOAT, val_unit: "unit_2".to_string(),
                    },
                    SensorColumn {
                        name: "col3".to_string(), val_type: ColumnType::STRING, val_unit: "unit_3".to_string(),
                    }],
                storage: SensorDataStorageCfg { variant: SensorDataStorageType::Default, params: None }
            }
        }

        test_invalid_auth("/api/sensors/create", Method::POST, Some(create_request("NewSensorName".to_string())), &state, &app).await;

        // --- Create sensor as John - Should succeed ---

        let token = login(&john().email, &john().password, &app).await;

        let sensor_info = create_request("NewSensorName".to_string());

        let sensor_name = sensor_info.name.clone();
        let sensor_descr = sensor_info.description.clone();
        let sensor_pos = sensor_info.position.clone();

        let body = execute_request("/api/sensors/create", Method::POST, None,
                                Some(sensor_info), Some(token.clone()),
                                StatusCode::OK, &app).await;

        // Check if sensor data is correct

        let resp: GenericUuidResponse = serde_json::from_value(body).unwrap();
        let sensor_id = uuid::Uuid::parse_str(&resp.uuid).unwrap();

        let sensor = cache::request_sensor(sensor_id, &state).await.unwrap();

        assert!(sensor.name.eq(&sensor_name) 
            && sensor.description.eq(&sensor_descr) 
            && sensor.position.eq(&sensor_pos) 
            && sensor.owner.unwrap() == john().id 
            && sensor.storage_type == SensorDataStorageType::Default);

        // --- Create sensor again as John - Should fail ---

        let sensor_info = create_request("NewSensorName".to_string());

        let _ = execute_request("/api/sensors/create", Method::POST, None,
                                Some(sensor_info), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_edit_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let target_sensor_own = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();
        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor2").unwrap();
        let target_sensor_not_allowed = test_sens.iter().find(|(name, _)| name == "MySensor4").unwrap();

        fn edit_request(name: String) -> EditSensorRequest {
            // Sensor of john which now revokes permissions for anne's role (system_test_role)
            EditSensorRequest {
                name: name,
                description: Some("My new sensor!".to_string()),
                position: Some((50.0, 10.0)),
                permissions: vec![SensorPermissionRequest { role_id: ROLE_SYSTEM_USER, operations: vec![DBOperation::READ]},
                    SensorPermissionRequest { role_id: TEST_SYS_ROLE, operations: vec![]}],
                storage: SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferCount, params: json!({"count": 10}).as_object().cloned() }
            }
        }

        test_invalid_auth(format!("/api/sensors/{}/edit", target_sensor_own.1).as_str(), Method::POST, Some(edit_request("MyNewName".to_string())), &state, &app).await;

        // --- Edit allowed (own) sensor as anne - Should succeed ---

        let anne_token = login(&anne().email, &anne().password, &app).await;

        let sensor_info = edit_request("MyNewName".to_string());

        let sensor_name = sensor_info.name.clone();
        let sensor_descr = sensor_info.description.clone();
        let sensor_pos = sensor_info.position.clone();
        
        let _ = execute_request(&format!("/api/sensors/{}/edit", target_sensor_allowed.1), Method::POST, None,
                                Some(sensor_info), Some(anne_token.clone()),
                                StatusCode::OK, &app).await;

        // Check if new sensor data is correct

        let sensor = cache::request_sensor(target_sensor_allowed.1, &state).await.unwrap();

        assert!(sensor.name.eq(&sensor_name) && sensor.description.eq(&sensor_descr) && sensor.position.eq(&sensor_pos)
            && sensor.storage_type == SensorDataStorageType::RingBufferCount && sensor.storage_params == json!({"count": 10}).as_object().cloned());

        // --- Check if john is now not able anymore to read sensor info (permission removed) - Should fail ---

        let token = login(&john().email, &john().password, &app).await;
        
        let _ = execute_request(&format!("/api/sensors/{}/info", &target_sensor_allowed.1), Method::GET, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        
        // Check if his API keys where removed for this sensor (had 2, now should have none valid)
        let john_keys: Vec<ApiKey> = test_keys.iter().filter(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1).cloned().collect();
        
        assert_eq!(john_keys.len(), 2);

        for k in john_keys {
            assert!(cache::request_api_key(k.id, &state).await.is_none());
        }

        // --- Edit not-allowed sensor as john - Should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/edit", target_sensor_not_allowed.1), Method::POST, None,
                                Some(edit_request("MyNewName".to_string())), Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Edit not-allowed sensor as john (admin) - Should succeed ---

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/sensors/{}/edit", target_sensor_not_allowed.1), Method::POST, None,
                                Some(edit_request("MyNewName2".to_string())), Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Edit non-existing sensor as john (admin) - Should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/edit", Uuid::new_v4()), Method::POST, None,
                                Some(edit_request("MyNewName2".to_string())), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_sensor(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;

        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();
        let target_sensor_not_allowed = test_sens.iter().find(|(name, _)| name == "MySensor4").unwrap();

        test_invalid_auth(format!("/api/sensors/{}/delete", target_sensor_allowed.1).as_str(), Method::DELETE, None::<Value>, &state, &app).await;

        // --- Delete allowed (his own) sensor as john - Should succeed ---

        let token = login(&john().email, &john().password, &app).await;

        let _ = execute_request(&format!("/api/sensors/{}/delete", target_sensor_allowed.1), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Delete allowed (his own) sensor as john again - Should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/delete", target_sensor_allowed.1), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
        
        // --- Delete un-allowed sensor as john - Should fail ---
        
        let _ = execute_request(&format!("/api/sensors/{}/delete", target_sensor_not_allowed.1), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Delete un-allowed sensor as john (as admin) - Should succeed ---

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/sensors/{}/delete", target_sensor_not_allowed.1), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Delete non-existing sensor as john (as admin) - Should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/delete", Uuid::new_v4()), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_create_sensor_api_key(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;

        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();
        let target_sensor_not_allowed = test_sens.iter().find(|(name, _)| name == "MySensor4").unwrap();
        
        let payload = CreateApiKeyRequest { name: "MyTestKey".to_string(), operation: DBOperation::READ };

        test_invalid_auth(format!("/api/sensors/{}/api_key/create", target_sensor_allowed.1).as_str(), Method::POST, Some(payload.clone()), &state, &app).await;

        // --- Create allowed sensor key as john - Should succeed ---

        let token = login(&john().email, &john().password, &app).await;

        let body = execute_request(&format!("/api/sensors/{}/api_key/create", target_sensor_allowed.1), Method::POST, None,
                                Some(payload.clone()), Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        let key: ApiKey = serde_json::from_value(body).unwrap();
        
        // Validate correctness of key
        assert!(key.sensor_id == target_sensor_allowed.1 && key.user_id == john().id && key.operation == payload.operation && key.name == payload.name);

        // --- Create not allowed sensor key as john - Should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/api_key/create", target_sensor_not_allowed.1), Method::POST, None,
                                Some(payload.clone()), Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Create not allowed operation for allowed sensor as john - Should fail ---

        let payload_inv = CreateApiKeyRequest { name: "MyTestKey2".to_string(), operation: DBOperation::INFO };

        let _ = execute_request(&format!("/api/sensors/{}/api_key/create", target_sensor_allowed.1), Method::POST, None,
                                Some(payload_inv.clone()), Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Create un-allowed sensor as john (as admin) - Should succeed ---

        role_db::assign_role(john().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make John admin!");

        let _ = execute_request(&format!("/api/sensors/{}/api_key/create", target_sensor_not_allowed.1), Method::POST, None,
                                Some(payload.clone()), Some(token.clone()),
                                StatusCode::OK, &app).await;

        // --- Create key for non-existent sensor - Should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/api_key/create", Uuid::new_v4()), Method::POST, None,
                                Some(payload.clone()), Some(token.clone()),
                                StatusCode::INTERNAL_SERVER_ERROR, &app).await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_sensor_api_key(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let test_keys = create_test_api_keys(&state).await;

        let target_sensor_allowed = test_sens.iter().find(|(name, _)| name == "MySensor").unwrap();

        let john_key_read_allowed = test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1 && k.operation == DBOperation::READ).unwrap().id;
        let john_key_write_allowed = test_keys.iter().find(|k| k.user_id == john().id && k.sensor_id == target_sensor_allowed.1 && k.operation == DBOperation::WRITE).unwrap().id;
        
        test_invalid_auth(format!("/api/sensors/{}/api_key/{}/delete", target_sensor_allowed.1, john_key_read_allowed).as_str(), Method::DELETE, None::<Value>, &state, &app).await;

        // --- Delete allowed sensor key as john - Should succeed ---

        let token = login(&john().email, &john().password, &app).await;

        let _ = execute_request(&format!("/api/sensors/{}/api_key/{}/delete", target_sensor_allowed.1, john_key_read_allowed), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        // Check if key really deleted
        assert!(cache::request_api_key(john_key_read_allowed, &state).await.is_none());

        // --- Delete allowed sensor key as john again - Should fail ---
        
        let _ = execute_request(&format!("/api/sensors/{}/api_key/{}/delete", target_sensor_allowed.1, john_key_read_allowed), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;
        
        // --- Delete not existing key as john - Should fail ---

        let _ = execute_request(&format!("/api/sensors/{}/api_key/{}/delete", target_sensor_allowed.1, Uuid::new_v4()), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Delete not allowed key as anne - Should fail ----

        let token = login(&anne().email, &anne().password, &app).await;

        let _ = execute_request(&format!("/api/sensors/{}/api_key/{}/delete", target_sensor_allowed.1, john_key_write_allowed), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::UNAUTHORIZED, &app).await;

        // --- Delete not allowed key as anne (admin) - Should succeed ----

        role_db::assign_role(anne().id, ROLE_SYSTEM_ADMIN, true, &state).await.expect("Failed to make Anne admin!");

        let _ = execute_request(&format!("/api/sensors/{}/api_key/{}/delete", target_sensor_allowed.1, john_key_write_allowed), Method::DELETE, None,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
    }
}