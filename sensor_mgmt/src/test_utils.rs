#[cfg(test)]
pub mod tests {
    use crate::authentication::token::generate_jwt_token;
    use crate::authentication::token_cache::register_token;
    use crate::authentication::{token, token_cache};
    use crate::database::models::api_key::ApiKey;
    use crate::database::models::db_structs::DBOperation;
    use crate::database::models::role::ROLE_SYSTEM_GUEST;
    use crate::database::models::sensor::{ColumnIngest, ColumnType, SensorColumn};
    use crate::database::{data_db, sensor_db, user_db};
    use crate::features::cache;
    use crate::features::sensor_data_storage::{SensorDataStorageCfg, SensorDataStorageType};
    use crate::features::user_sens_perm::UserSensorPerm;
    use crate::handler::main_hdl::config;
    use crate::handler::models::requests::{
        CreateApiKeyRequest, CreateSensorRequest, SensorDataIngestEntry, SensorPermissionRequest,
    };
    use crate::state::{init_app_state, AppState};
    use actix_http::body::BoxBody;
    use actix_http::{header, Method, Request};
    use actix_web::dev::{Service, ServiceResponse};
    use actix_web::http::header::{Accept, ContentType};
    use actix_web::http::StatusCode;
    use actix_web::test::TestRequest;
    use actix_web::{http, test, web, App};
    use serde::Serialize;
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use std::sync::Arc;
    use uuid::{uuid, Uuid};

    pub struct TestUser {
        pub id: Uuid,
        pub name: String,
        pub email: String,
        pub verified: bool,
    }

    pub struct TestSensor {
        pub name: String,
        pub owner: Option<Uuid>,
        pub permissions: Vec<SensorPermissionRequest>,
    }

    pub fn john() -> TestUser {
        TestUser {
            id: uuid!("587EED02-3829-4660-B7FD-B02743C3941A"),
            email: "john@gmail.com".to_string(),
            name: "John Doe".to_string(),
            verified: true,
        }
    }

    pub fn anne() -> TestUser {
        TestUser {
            id: uuid!("1DB2CE41-9748-4AB7-9A4B-68CF14D0DD0F"),
            email: "anne@gmail.com".to_string(),
            name: "Anne Clark".to_string(),
            verified: true,
        }
    }

    pub fn jack() -> TestUser {
        TestUser {
            id: uuid!("58d52d19-e08a-4aa7-85d8-f955b18431fe"),
            email: "jack@gmail.com".to_string(),
            name: "Jack Frags".to_string(),
            verified: true,
        }
    }

    /// NOTE jane is a not verified user account
    pub fn jane() -> TestUser {
        TestUser {
            id: uuid!("765EED02-3829-4660-B7FD-B02743C3941A"),
            email: "jane@gmail.com".to_string(),
            name: "Jane Dane".to_string(),
            verified: false,
        }
    }

    pub const TEST_SYS_ROLE: Uuid = uuid!("1e804d35-c8e3-49ee-86d4-3e556a82a1af");
    pub const TEST_ROLE: Uuid = uuid!("2e804d35-c8e3-49ee-86d4-3e556a82a1af");
    pub const TEST_SYS_ROLE2: Uuid = uuid!("3e804d35-c8e3-49ee-86d4-3e556a82a1af");
    pub const TEST_ROLE2: Uuid = uuid!("4e804d35-c8e3-49ee-86d4-3e556a82a1af");

    pub const TEST_ROLE_THAT_NOT_EXISTS_BUT_IS_VALID: Uuid =
        uuid!("5e804d35-c8e3-49ee-86d4-3e556a82a1af");

    pub async fn create_test_app(
        pool: PgPool,
    ) -> (
        impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
        AppState,
    ) {
        let state = init_app_state(pool);

        let app = App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(config);

        let app = test::init_service(app).await;

        (app, state)
    }

    fn test_sensors() -> Vec<TestSensor> {
        // John's sensor that no one has access to
        let test1 = TestSensor {
            name: "MySensor".to_string(),
            owner: Some(john().id),
            permissions: vec![],
        };

        // Anne's sensor that john has INFO, READ, WRITE access to
        let test2 = TestSensor {
            name: "MySensor2".to_string(),
            owner: Some(anne().id),
            permissions: vec![SensorPermissionRequest {
                role_id: TEST_SYS_ROLE,
                operations: vec![DBOperation::INFO, DBOperation::READ, DBOperation::WRITE],
            }],
        };

        // System sensor that john/anne has INFO access to
        let test3 = TestSensor {
            name: "MySensor3".to_string(),
            owner: None,
            permissions: vec![SensorPermissionRequest {
                role_id: TEST_ROLE,
                operations: vec![DBOperation::INFO],
            }],
        };

        // System sensor that john/anne has no access to
        let test4 = TestSensor {
            name: "MySensor4".to_string(),
            owner: None,
            permissions: vec![],
        };

        // System sensor with public access
        let test5 = TestSensor {
            name: "MySensor5".to_string(),
            owner: None,
            permissions: vec![SensorPermissionRequest {
                role_id: ROLE_SYSTEM_GUEST,
                operations: vec![DBOperation::INFO, DBOperation::READ, DBOperation::WRITE],
            }],
        };

        vec![test1, test2, test3, test4, test5]
    }

    pub async fn create_test_sensors(state: &AppState) -> Vec<(String, Uuid)> {
        let sensors = test_sensors();
        let mut res: Vec<(String, Uuid)> = Vec::new();

        for sensor in sensors {
            let cr = CreateSensorRequest {
                name: sensor.name.clone(),
                position: None,
                description: None,
                permissions: sensor.permissions.clone(),
                columns: vec![
                    SensorColumn {
                        name: "col1".to_string(),
                        val_type: ColumnType::INT,
                        val_unit: "unit_1".to_string(),
                        val_ingest: ColumnIngest::LITERAL,
                    },
                    SensorColumn {
                        name: "col2".to_string(),
                        val_type: ColumnType::FLOAT,
                        val_unit: "unit_2".to_string(),
                        val_ingest: ColumnIngest::LITERAL,
                    },
                    SensorColumn {
                        name: "col3".to_string(),
                        val_type: ColumnType::STRING,
                        val_unit: "unit_3".to_string(),
                        val_ingest: ColumnIngest::LITERAL,
                    },
                ],
                storage: SensorDataStorageCfg {
                    variant: SensorDataStorageType::Default,
                    params: None,
                },
            };

            let new_sensor = sensor_db::create_sensor(cr, sensor.owner, &state)
                .await
                .unwrap();

            res.push((sensor.name, Uuid::parse_str(&new_sensor.uuid).unwrap()));
        }

        res
    }

    /// Creates an API key for each user for each sensor he has access (READ, WRITE) to.
    pub async fn create_test_api_keys(state: &AppState) -> Vec<ApiKey> {
        let mut res: Vec<ApiKey> = Vec::new();

        let sensors = sensor_db::get_sensor_overview(&state).await.unwrap();
        let users = user_db::user_list(&state).await.unwrap();

        for sensor in sensors {
            let full_sensor = cache::request_sensor(sensor.id, &state).await.unwrap();

            for user in users.iter() {
                let perms =
                    sensor_db::get_user_sensor_permissions(Some(user.id), &full_sensor, &state)
                        .await;

                if perms.has(UserSensorPerm::Read) {
                    res.push(
                        sensor_db::create_api_key(
                            sensor.id,
                            user.id,
                            CreateApiKeyRequest {
                                name: "TestKeyRead".to_string(),
                                operation: DBOperation::READ,
                            },
                            &state,
                        )
                        .await
                        .unwrap(),
                    );
                }

                if perms.has(UserSensorPerm::Write) {
                    res.push(
                        sensor_db::create_api_key(
                            sensor.id,
                            user.id,
                            CreateApiKeyRequest {
                                name: "TestKeyWrite".to_string(),
                                operation: DBOperation::WRITE,
                            },
                            &state,
                        )
                        .await
                        .unwrap(),
                    );
                }
            }
        }

        res
    }

    pub async fn add_dummy_data(
        amount: u32,
        index_off: u32,
        sleep_ms: u64,
        sensor_id: Uuid,
        state: &AppState,
    ) -> Vec<Value> {
        let mut payloads: Vec<Value> = Vec::new();

        for i in 0..amount {
            let index = i + index_off;

            let payload = json!({
                "col1": index,
                "col2": 1.11 * index as f64,
                "col3": format!("Hello{}", "o".repeat(index as usize)),
            });

            let entry = SensorDataIngestEntry::from_json(payload.clone(), None);

            let sensor = Arc::new(
                cache::request_sensor(sensor_id, &state.clone())
                    .await
                    .unwrap(),
            );

            data_db::add_sensor_data(sensor, &vec![entry], state.clone())
                .await
                .unwrap();

            tokio::time::sleep(core::time::Duration::from_millis(sleep_ms)).await;

            payloads.push(payload);
        }

        payloads
    }

    pub async fn execute_request<T>(
        api_path: &str,
        method: Method,
        params: Option<Vec<(String, String)>>,
        payload: Option<T>,
        token: Option<String>,
        expected_status: StatusCode,
        app: impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
    ) -> Value
    where
        T: Serialize + Clone,
    {
        let create_request = || -> TestRequest {
            match method {
                Method::GET => TestRequest::get(),
                Method::POST => TestRequest::post(),
                Method::PUT => TestRequest::put(),
                Method::DELETE => TestRequest::delete(),
                Method::PATCH => TestRequest::patch(),
                _ => unreachable!(),
            }
        };

        // NOTE this could be a parameter if needed later on
        let expected_content_type = ContentType::json();

        let mut req = create_request().uri(api_path);

        // If we have query params present, parse them and add them to the uri
        if let Some(param_vec) = params {
            let new_api_path = format!(
                "{}?{}",
                api_path,
                serde_urlencoded::to_string(param_vec).unwrap()
            );
            req = create_request().uri(new_api_path.as_str());
        }

        match payload {
            None => {}
            Some(v) => {
                req = req.set_json(v);
            }
        }

        match token {
            None => {}
            Some(token) => {
                req = req.insert_header((http::header::AUTHORIZATION, token));
            }
        };

        // Tell the server that we want to always recieve a JSON response
        req = req.append_header(Accept::json());
        let resp: ServiceResponse = test::call_service(&app, req.to_request()).await;
        let resp_status_code = resp.status();

        // Check the content type header
        let mut is_expected_content_type = false;
        let ct_header = resp.headers().get(header::CONTENT_TYPE);
        let mut ct_header_str: String = "".to_string();
        match ct_header {
            None => {}
            Some(ct) => {
                is_expected_content_type =
                    ct.to_str().unwrap() == expected_content_type.to_string().as_str();
                ct_header_str = ct.to_str().unwrap().to_owned();
            }
        }

        let body_bytes = test::read_body(resp).await;
        if body_bytes.len() == 0 && resp_status_code == expected_status {
            return json!({});
        }
        let result = serde_json::from_slice(&body_bytes);
        let parsed_result = match result {
            Err(err) => {
                println!("url:  {}", api_path);
                println!(
                    "Status:    (is) {} ? {} (should be)",
                    resp_status_code, expected_status
                );
                println!(
                    "ContentType:   (is) {} ? {} (should be) => {}",
                    ct_header_str, expected_content_type, is_expected_content_type
                );
                println!("Response body:    '{:?}'", body_bytes.clone());
                panic!("Failed to deserialize as JSON body: {:?}", err);
            }
            Ok(d) => d,
        };

        // Assertion
        if resp_status_code != expected_status || !is_expected_content_type {
            println!("url:  {}", api_path);
            println!(
                "Status:    (is) {} ? {} (should be)",
                resp_status_code, expected_status
            );
            println!(
                "ContentType:   (is) {} ? {} (should be) => {}",
                ct_header_str, expected_content_type, is_expected_content_type
            );
            println!("{}", parsed_result);
            panic!("Either status or contentType are unexpected!");
        }

        parsed_result
    }

    pub async fn login(user: &TestUser, state: &AppState) -> String {
        // Anstatt den login Endpunkt anrufen
        // Einfach direkt token generien und registireren
        // Authentication macht ab jetzt nicht mehr SensBee, wir imitieren einen erfolgreichen call an einen IDP

        let token = match generate_jwt_token(user.id, state.jwt.max_age, &state.jwt.private_key) {
            Ok(t) => t,
            Err(err) => panic!("failed to create jwt with: {}", err),
        };
        register_token(token.token_uuid, token.clone());

        token.token.unwrap()
    }

    pub async fn logout(
        token: String,
        app: impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
    ) -> () {
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
    }

    pub async fn test_invalid_auth<T: Serialize + Clone>(
        api_path: &str,
        method: Method,
        payload: Option<T>,
        state: &AppState,
        app: impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
    ) {
        // --- Perform API call without token - Should fail ---

        let _ = execute_request(
            api_path,
            method.clone(),
            None,
            payload.clone(),
            None,
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Perform API call with invalid token - Should fail ---

        let _ = execute_request(
            api_path,
            method.clone(),
            None,
            payload.clone(),
            Some("invalid_token".to_string()),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Perform API call with expired token - Should fail ---

        let token_details =
            token::generate_jwt_token(jane().id, -100, &state.jwt.private_key).unwrap();

        token_cache::register_token(token_details.token_uuid, token_details.clone()); // Register to be able to remove it during request

        let _ = execute_request(
            api_path,
            method.clone(),
            None,
            payload.clone(),
            Some(token_details.token.unwrap()),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // Check if expired token was removed

        assert!(!token_cache::has_token(token_details.token_uuid).0);

        // --- Perform API call with token created by invalid secret - Should fail ---

        let token_details = token::generate_jwt_token(
            jane().id,
            state.jwt.max_age,
            &"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDvTZ0LKRtxaleC
xpU+QpN9iYQwPJtl0FNhTVZnYjMlDZscqsv+xi62Hr5CiQHpvAkNfVJ/yZ/TeN1J
IUiMNB7oQimvysCdvalm1gGtZZnQ1T0SlmYYpHXodbA/T5zITkdfdhvQeFsVmhvp
B0PsXol5IZmSFecK737ulX2bRjCZmFa2WbEM1aV+0vchYBCSIgLHEWySN3mG8JQ3
dqZIyIlPaGBTeBUVjBTzIhBg4XHJKPuyTwcGMlZQBE1ORO4uHCMKD9PRIU8oY/MH
5VX2PaszRkQZ4AU0+f26R7k3gfg9c8Cs1r3F0hNVFjWGCHF3e7CSoOe5/rzgEQF0
+yuUz39pAgMBAAECggEAC5VN6/egKq/7Tur2V+ZolbvFkIEqg3XfR1czPrtX3uwG
7U8OI0WsBqg7zOQtWcc+h+7gQqu7hwSzb2IDTTgLo/Hp6yatBqWi0MW8nIxNsvhT
ZbYueHRjea5SqunbXK2/Ui1ZINDmlcfZIIE3xjX4QQsBkDrrrVGU6w8E3rJ50UFg
wIbiRdBYNWulJlNjS0QchNRSFz3+6eILTFJAKO4dK7SqWsZBHZjLhjORFDUlzKDD
5+mDAOCs8P6J/7vd4OxCPNiXRkNxRsh1gvjZO3KQJ3AMYYOe9ohFmNYNew+pkTwu
fRoVhZc7XmG81SaJit3mwR6Kv4eAcz0fIp9iVnXU1QKBgQD6p0OOGHL+8SUS2ABb
S0nOA44OwMafQ3ahPq7ZxmuR/Ws2bkYa1KZO7yRr+2U8zLstjf3frenOkykC40vq
2opKY+j9mhGx4Ys0TlYClTjOlwkCx2qCTSxcdA1FaEAE6c5xPs04ie4KB1GN/S/2
lBe8NuF7bY1mQLwGSLprbyGdZQKBgQD0aF6ugk6DoR6aglhLU1xUPc52z1DDCCUt
9Dy22M+XkHa7qgiRNusMsFFizkFv0GwRRRFOiJaAOAvkiFFj8OxbvMgbCym3sA9q
YQDLF+vlqzaC3ZCwWOe/AOUltAKHvfKaRrXYTOC1862tk22EW6hrJZgsW8FqF4x4
8anOBRprtQKBgQDXBvL/TZ4pc3oYhlEYAKiaIZaWtW4vZtK4VWvuyzexEDQPh96A
WfkqMiGOuSYKWKAi3nLyluHDI5/FKHUSTtTgKIHSPX/8l76x6poCsT0AjbVfOu/2
RHpP/gb8igiRrno50GSBomIhHFIsew3QfQ83meUp27u4AsTKp021qKqvuQKBgCpD
piPdSsB+azFi2uvjtXKn4X0wKpIfZXaF5r3jzjoydCXNqH+cFJd0Ig7JBg3U5+sw
m2aOPiBcEMprPE/hCK5wfdYXXxZxrqjBr4ZvU466xclpkSy9ow2nlPipIUrh8QL2
uVl3KeCtC9qZRPX/d6dXr/HzyAWVnugHOkrzHPeFAoGBALoTlh9NYwPURJZHQip5
gxlqxK8h0fs4wDg8l1ZYD0ytgv2buEEGt7BOpzSlEgBtmbUk6s/DYjV+qhWki1Z1
on9rI3welIcf2R5ZfxQxIVkX/TT6hYi9GI7jGcZJWNyybsfOKWnMt73FONRiGMau
Qlz0xo1NyHqXj/hqUW+eENkV
-----END PRIVATE KEY-----"
                .as_bytes()
                .to_owned(),
        )
        .unwrap();

        let _ = execute_request(
            api_path,
            method.clone(),
            None,
            payload.clone(),
            Some(token_details.token.unwrap()),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Perform API call with valid token of non-existent user - Should fail ---

        let token_details =
            token::generate_jwt_token(Uuid::new_v4(), state.jwt.max_age, &state.jwt.private_key)
                .unwrap();

        let _ = execute_request(
            api_path,
            method.clone(),
            None,
            payload.clone(),
            Some(token_details.token.unwrap()),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Perform API call with token of non-verified user - Should fail ---

        let token_details =
            token::generate_jwt_token(jane().id, state.jwt.max_age, &state.jwt.private_key)
                .unwrap();

        let _ = execute_request(
            api_path,
            method.clone(),
            None,
            payload.clone(),
            Some(token_details.token.unwrap()),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;

        // --- Perform API call with user that logged out - Should fail ---

        let token = login(&john(), state).await;

        logout(token.clone(), &app).await;

        let _ = execute_request(
            api_path,
            method.clone(),
            None,
            payload.clone(),
            Some(token),
            StatusCode::UNAUTHORIZED,
            &app,
        )
        .await;
    }

    // Event handler

    pub const TEST_EVENT_HANDLER: Uuid = uuid!("e1065bd4-b304-435b-92cf-63c270b722c3");
}
