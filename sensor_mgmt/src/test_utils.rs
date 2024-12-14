#[cfg(test)]
pub mod tests {
    use actix_http::body::BoxBody;
    use actix_http::{Method, Request};
    use actix_web::{body::to_bytes, http, test, web, App};
    use actix_web::dev::{Service, ServiceResponse};
    use actix_web::http::StatusCode;
    use actix_web::test::TestRequest;
    use serde_json::{json, Value};
    use serde::Serialize;
    use sqlx::PgPool;
    use uuid::{uuid, Uuid};
    use crate::authentication::{token, token_cache};
    use crate::features::cache;
    use crate::database::models::db_structs::DBOperation;
    use crate::database::models::role::ROLE_SYSTEM_GUEST;
    use crate::database::models::sensor::{ColumnType, SensorColumn};
    use crate::database::{sensor_db, user_db};
    use crate::database::models::api_key::ApiKey;
    use crate::features::sensor_data_storage::{SensorDataStorageCfg, SensorDataStorageType};
    use crate::handler::models::requests::{CreateApiKeyRequest, CreateSensorRequest, SensorPermissionRequest};
    use crate::features::user_sens_perm::UserSensorPerm;
    use crate::handler::cli_hdl;
    use crate::handler::main_hdl::config;
    use crate::state::{AppState, JWTConfig};

    pub struct TestUser {
        pub id: Uuid,
        pub name: String,
        pub email: String,
        pub password: String,
        pub verified: bool
    }

    pub struct TestSensor {
        pub name: String,
        pub owner: Option<Uuid>,
        pub permissions: Vec<SensorPermissionRequest>
    }

    pub fn john() -> TestUser {
        TestUser {
            id: uuid!("587EED02-3829-4660-B7FD-B02743C3941A"),
            email: "john@gmail.com".to_string(),
            name: "John Doe".to_string(),
            password: "MySecret".to_string(),
            verified: true
        }
    }

    pub fn anne() -> TestUser {
        TestUser {
            id: uuid!("1DB2CE41-9748-4AB7-9A4B-68CF14D0DD0F"),
            email: "anne@gmail.com".to_string(),
            name: "Anne Clark".to_string(),
            password: "MySecret".to_string(),
            verified: true
        }
    }

    pub fn jane() -> TestUser {
        // Not verified yet
        TestUser {
            id: uuid!("765EED02-3829-4660-B7FD-B02743C3941A"),
            email: "jane@gmail.com".to_string(),
            name: "Jane Dane".to_string(),
            password: "MySecret".to_string(),
            verified: false
        }
    }

    pub async fn create_test_app(pool: PgPool) -> (impl Service<Request, Response=ServiceResponse<BoxBody>, Error=actix_web::Error>, AppState) {
        let state = AppState{ db: pool.clone(), jwt: JWTConfig::init() };

        let app = App::new().app_data(web::Data::new(state.clone())).configure(config);

        let app = test::init_service(app).await;

        (app, state)
    }

    pub async fn create_test_app_cli() -> impl Service<Request, Response=ServiceResponse<BoxBody>, Error=actix_web::Error> {
        let app = App::new().configure(cli_hdl::config);

        let app = test::init_service(app).await;

        app
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
            permissions: vec![SensorPermissionRequest { role_name: "system_test_role".to_string(), operations: vec![DBOperation::INFO, DBOperation::READ, DBOperation::WRITE] }],
        };

        // System sensor that john/anne has INFO access to
        let test3 = TestSensor {
            name: "MySensor3".to_string(),
            owner: None,
            permissions: vec![SensorPermissionRequest { role_name: "test_role".to_string(), operations: vec![DBOperation::INFO] }],
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
            permissions: vec![SensorPermissionRequest { role_name: ROLE_SYSTEM_GUEST.to_string(), operations: vec![DBOperation::INFO, DBOperation::READ, DBOperation::WRITE] }],
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
                    },
                    SensorColumn {
                        name: "col2".to_string(),
                        val_type: ColumnType::FLOAT,
                        val_unit: "unit_2".to_string(),
                    },
                    SensorColumn {
                        name: "col3".to_string(),
                        val_type: ColumnType::STRING,
                        val_unit: "unit_3".to_string(),
                    }],
                storage: SensorDataStorageCfg { variant: SensorDataStorageType::Default, params: None }
            };

            let new_sensor = sensor_db::create_sensor(cr, sensor.owner, &state).await.unwrap();

            res.push((sensor.name, new_sensor));
        }

        res
    }

    /// Creates an API key for each user for each sensor he has access (READ, WRITE) to.
    pub async fn create_test_api_keys(state: &AppState) -> Vec<ApiKey>{
        let mut res: Vec<ApiKey> = Vec::new();
        
        let sensors = sensor_db::get_sensor_overview(&state).await.unwrap();
        let users = user_db::user_list(&state).await.unwrap();
        
        for sensor in sensors {
            let full_sensor = cache::request_sensor(sensor.id, &state).await.unwrap();

            for user in users.iter() {
                let perms = sensor_db::get_user_sensor_permissions(Some(user.id), &full_sensor, &state).await;

                if perms.has(UserSensorPerm::Read) {
                    res.push(sensor_db::create_api_key(sensor.id, user.id,
                                              CreateApiKeyRequest {
                                                  name: "TestKeyRead".to_string(),
                                                  operation: DBOperation::READ,
                                              }, &state).await.unwrap());
                }

                if perms.has(UserSensorPerm::Write) {
                    res.push(sensor_db::create_api_key(sensor.id, user.id,
                                              CreateApiKeyRequest {
                                                  name: "TestKeyWrite".to_string(),
                                                  operation: DBOperation::WRITE,
                                              }, &state).await.unwrap());
                }
            }
        }
        
        res
    }
    
    pub async fn execute_request<T>(api_path: &str, method: Method,
                                    payload: Option<T>, token: Option<String>,
                                    expected_status: StatusCode,
                                    app: impl Service<Request, Response=ServiceResponse<BoxBody>, Error=actix_web::Error>)
                                    -> Value where T: Serialize + Clone {
        
        let create_request = || -> TestRequest {
            match method {
                Method::GET => TestRequest::get(),
                Method::POST => TestRequest::post(),
                Method::PUT => TestRequest::put(),
                Method::DELETE => TestRequest::delete(),
                Method::PATCH => TestRequest::patch(),
                _ => unreachable!()
            }
        };
        
        let req = match token {
            Some(token) => create_request()
                .uri(api_path)
                .set_json(payload.clone().map(|x| x))
                .insert_header((
                    http::header::AUTHORIZATION,
                    token,
                ))
                .to_request(),
            
            None => create_request()
                .uri(api_path)
                .set_json(payload.clone().map(|x| x))
                .to_request()
        };
        
        let resp = test::call_service(&app, req).await;

        let status = resp.status();

        let body_bytes = to_bytes(resp.into_body()).await;
        
        if body_bytes.is_ok() {
            let result = serde_json::from_slice(&body_bytes.unwrap());
            
            if result.is_ok() {
                let result = result.unwrap();
                
                if status != expected_status { println!("{:?}", result); }

                assert_eq!(status, expected_status);

                return result;
            }
        }

        assert_eq!(status, expected_status);
        
        Value::default()
    }
    
    pub async fn login(email: &str, password: &str, app: impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>) -> String {
        let payload = json!({
                "email": email,
                "password": password,
            });
        
        let body = execute_request("/auth/login", Method::POST,
                                Some(payload), None,
                                StatusCode::OK, &app).await;

        // check token
        let token = body.as_str().unwrap();

        token.to_string()
    }

    pub async fn test_invalid_auth<T: Serialize + Clone>(api_path: &str, method: Method, payload: Option<T>, state: &AppState,
                                                         app: impl Service<Request, Response=ServiceResponse<BoxBody>, Error=actix_web::Error>) {

        // --- Perform API call without token - Should fail ---

        let _ = execute_request(api_path, method.clone(), payload.clone(), None, StatusCode::UNAUTHORIZED, &app).await;

        // --- Perform API call with invalid token - Should fail ---

        let _ = execute_request(api_path, method.clone(), payload.clone(), Some("invalid_token".to_string()), StatusCode::UNAUTHORIZED, &app).await;

        // --- Perform API call with expired token - Should fail ---
        
        let token_details = token::generate_jwt_token(jane().id, -100, state.jwt.private_key.to_owned()).unwrap();
        
        token_cache::register_token(token_details.token_uuid, token_details.clone()); // Register to be able to remove it during request

        let _ = execute_request(api_path, method.clone(), payload.clone(), Some(token_details.token.unwrap()), StatusCode::UNAUTHORIZED, &app).await;
        
        // Check if expired token was removed

        assert!(!token_cache::has_token(token_details.token_uuid).0);

        // --- Perform API call with token created by invalid secret - Should fail ---

        let token_details = token::generate_jwt_token(jane().id, state.jwt.max_age, "LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUlKS0FJQkFBS0NBZ0VBME05dm5IWExKZFgzbk1hWXM4OHVvd1dRS21NSWRNMXVzbGN1MUhZdW01NWs1RE1yCm9pclBXcjcyQW5uVUhVZDczSmo2b3kzSGtYMmZrU3NGSkpVSitZdlQrL3RSRHpGdHlMWXJrbUxFVnJNbmVjVSsKeis0RHJVYitDdmkwUitXWmorMDRLdU1JdTNjSU5ONjh5ZWtQSjB4VVRQSm04bWNtT1ZGN1NJUVBxRXJKR3NtRgp2dTJZOEZGdmo5VkluK2Z3ZmRBeHJhRTEyem05WlhkWnloL2QvU05wZUgxWkVXYmVnSmhPTUJzWWlLcVhMS3V5Clc5bm5uRld2QUNTbGtoYjFLVlY0UW1TV0FVVnNnMEdTMGo5QlFrVkQ1TEZBVWpndDlzSzVDRWtxRGhpS1pNQXIKVFpWVU12eDcwTHRoZmpRNng0ZXljVEVNeG10dXRqam1kYXRQcmJFWGZleHNqNTRIVHlwSzNwSU1ncW1OVTFjNQpHYVFlcW5CSElNa0o2MTk1cUZ4WE5HejE5c2liQlkzTlpleE5HWmc0bkdGTjdrUW9QR2FWdHMyYXdiVU4xL0JZCnBjN0FpSnh5RFg5SkFweUFSUWgxcmxDVkdXb3daQ05WRkJ4OWNMTjBDeGpyYi9td0sxSkRmMHFmSms3QmpyVHcKTnVzL1k5NUp5TE1JSHNvTlpRYk1uL095N2pmMXVjV3dNUkRnYjhqSDdxa2tCQ2F3OW1md2djZVE0cVBtZzFsMgovMjVmQzh1eGlJdWRZWCtQZjBaSVVkQ09zTDllT2xYYWJGcTA4UG5jUmFuRzBFcHRsNnV6eTVuNi9waHdEK0R0Cmh1RE5ycURoNjVTUy9uU1JEVWRHbGtITms0RlByZGNRK0kraWtBZDM1RnJVb0l3ajRjT0VLa0JyT1Q4Q0F3RUEKQVFLQ0FnQnpCUkN4MnFEZ1lwQldwMzZON1YzL0pwMVcrOTQ0bU1DVk5EanpoM1g4K3E4UWxLOUFVTnlQWEFrZgpMQVNQYkVUcUtzcEZBSDZod2RVWG5kN2pXOFYyMUhNY3BqN3NZNG5adVo4ZXI1RC9RUWhKcDBFR1FGRitMVkRhCnNreDhIaGtNa3RzUnBLVzJ2Y2FqZU4zOVNvZXlXZlZGdlhDL3JkbjhVTW5jRkFLYjdUWUJyMmdnMTdnYkNJQ3YKZGdqZkxGL29yYm52cnBHQUJMb3pIaDh6bTRJb1lrMUN0YWxPVUovWHJnM0RxZWxGdnRJdkpSVEdTNjJ0Qy9XdAoyb0hwaXdQWWxOLzlrbktlbUtOQldlbUtMcFcvNzIrS2xhaWNvWjJRQTRydzZYeGs3MWVzVDc2S3Flc0xldENwCkZjNktPakwybmVUSlBQK1FmTFVyWXdSdlpNSXFKOVBVQjZUR1BIRVpsSmROQml5VlNya2d2S2R1NjllemJZZmgKQkRJeXh2Mnh4Q0pSTFU1VUJXb2I0YWp6RWlQZkhmSkIvUnNrOGdVNGNrc0Z2U0ZhZHpPU1hlNlZEYjNRR3NZNgozdFFlK2xsem5lOFVFWTg1NGg2L0JiRENWbHVEa2UxNTk5Ny8yam9MUnl0U0EySGxXc1N4MW41SFp5ZDZ1a1NpCkd4bXgvNHN6b2NGZ1FYVnhhMTljdVlIZXFSK2haa3FGaC9EYTh5UVNsOWRHYXh4WkF5RWplMzBWdjdIeEcxQ0MKQjM4RjZSUmh5Qm9LSnpRbnRNVlY2YXc2Q2FZMk43YS9hRFBLWjRONU5YY0dDKzZSRHh3b0M5bFNleXRrbkRCago1UWVIZmJMai9mRzhQWUU1NnRSWnNEZGNNVmg4SllDdk1acG1uUW9Qb0lUYU9PenNRUUtDQVFFQTl0bzZFOXhnCmZTa1NJMHpDYUdLNkQzMnBmdFJXbkR5QWJDVXpOYk5rcEJLOHJuSGlXanFJVDQzbVd6empGc0RvNlZwVXpscFQKYVVHWkNHMXc5THpHaWlaNllBd0F4TXBOZERzMFFOemhJNjMzS0tseHd0NGlUaGQ0aG9oYmZqdndGWHkyQ0paWgovUkkyZ1AwUEdvSENURXFMMTgzQklpYnJJR0g4dzY2K0F5cFc2L3cvdENEQ2NReFA1RE45YlNPSmFlQVI0a1NzCjg3REM1bmdNMVhJeVFpSCtvL21zaEpUS3ZhZUVpeTVmM1BaaExJNWZNQlZwN0tWTUNZY3V2NWZ4Y3pHVHZFM1YKcHcxamJmSzRDdG9xemFmK3hrdUk5ZWNjakp4TU5KRGc0QW5CNEpxWm11Y2dQWGJPdEpRR2VHaHZqZlBqTVZHZworTHhzSUFWZE8vRjFtUUtDQVFFQTJJeFNNK1VZOTFoem5vUURSbzV4WWVGS0dUeDhVZ00rWDdycURzTXp6NUVSCkRWKzh5WlNsY29NVjNlcGVSdjFHYlRodEUvTlZ4c1k2SW5yUkVJNHB2WFJqYkxqZDZPVkJYWENsYVl1YWsyV20KV2QxTVo4dDZRMUtVWXBFS0piZVRMN09SUmtibnIzTHhmWGJ2WTRPV1BaQjZyNktoaXljbTFubUNJU0hiMFh5Mwp1WHY1VVZEYVZWdklnS0RkNGhrRGZSWmEzNEZZUDYvcUFzMzkyWkJnclpvbVk0SkFMN2F0RnpmWVVZMUtlamV3CmpJWCtpQmRkdkd0cXQ0ZzYwQkgzQUxCZjJFb0Q4bkluaHRuUWtSd0d5QnRFN1pRVGdCYzRJbm5mR2tMZTRpWDkKQlZaSFgxb0VHWUp3RkVUNk1zUHFwcU8yWDhPT21YRDFFVFhUTUVjOGx3S0NBUUFmMWQwUG1xaEcrL2orM0hObQpDdlY3OGZUZUNueHhBY3grSmY0SXV1NEx5dTdTZ0pWMGxYL200cUlHdWo5L083bk4vbnhaY0lTNVdtQm1HZGNyCmVQMFI3QXgwUHBnS3lSeGNGUmFVRnVoaU5abGVnUnZPeWQ4YXV5UXNGWUhYTWR1d3FiakFPc080UTVVTDVaY0IKRUNNQ3U4cDFObS9sKzZidk1qUHErS3BBdGtFbmhneWhLbWhwTS9GSnVPcEFIUWtud21JTUVGZE54a29jZHZjUQp2LzJEVWVjSk5yWHRFMU5pU2l4cDFyMCtQZmdpU3VvenhVODMyY21Jb1FxQ1l4SWNqUlJFZ0xWQktoVGNwU1RmCklXdkx3aEsxZUNCZHRrU1VUY1AyTTRrTTI3VkpSaWJ4TjBXTko3bFl5STVkRVByeUQ3WUpNa0hVVWxpUGVLR2gKalc1aEFvSUJBQWdWQktSbk1vMVl3Y2Z5eVdTQ3dIeVV1ZjFESXFpMDhra0VZdVAyS1NMZ0dURFVsK2sySVE2cgpFYy9jaFhSRTA3SVQzdzVWa0tnQWtmN2pjcFlabURrMzlOWUQrRlJPNmllZ29xdlR5QXNrU2hja2lVdCticXZBCmswVXlnSnh6dzR5T09TZlVVYVZjdHVLbDQ3MWxGZUJxV2duZ0dnTmxqSytJalhETElMY3EzbmlQeGZoZytpVWgKYmRSUExMalpraVhEQmRVOXNKdC81MDMvZmkvMmtZVXBNYkdaRk9neSt6YllvTHc2ZDhNai9QVGhzMlJFNnZ5egpUYUpYOVVuNndhdEc2ZXphcGxjUUo2V0N6NlA2MWMzMkpwWnZabUxyZXU3ZWVaTXpWN285RExwOFErR3RMR1gvClZrdUxYNE14aUxwN2RiMFJRV3M4cWdqZ1oyZHY0VFVDZ2dFQkFMRjRiNnhRNjJJaCtMaTdsVk9lSWQ5VFVub08KUU1LUVNRN0xlWjJ4TmhCYWRPUEt0ZmJ5U0dGMGZieXZiVWk2czAyVnJpWC93S1V6T2o1WEFUbUZYQVdzYnU1dwo2M1JVR09ua2Z6cjIwWDZJWTVzOS9kdnJWZXFLNkpLdlQyZ0F0dWMwNXNCZzJPaG5CdHh2c0JDekhYVy9YRWJsCktWamVIMUxQTnZMaFNSc3BvT2FFVUhlaHpNN2c1V3FGSXhSQmRlb2J1SWNxQ1J2WjRFZGl6b05ybzVRZXFub3oKMTlyU0VVcTNBMEdIdE5Pb0xuV2Q3ZkZta2NOMEw5S3R0MTdsK2wxV0c3Y2kxVTVuSXBlOXBxZThlUUU2YmNYaApkNnlkdWd3UUpXbUxKSlpMQUs3eFpZdzd1ODhoa3ppZ2pSR2ltWHZ4VTJCMTU5OW5OT2NrNWQ0YXJTRT0KLS0tLS1FTkQgUlNBIFBSSVZBVEUgS0VZLS0tLS0=".to_string()).unwrap();

        let _ = execute_request(api_path, method.clone(), payload.clone(), Some(token_details.token.unwrap()), StatusCode::UNAUTHORIZED, &app).await;

        // --- Perform API call with valid token of non-existent user - Should fail ---
        
        let token_details = token::generate_jwt_token(Uuid::new_v4(), state.jwt.max_age, state.jwt.private_key.to_owned()).unwrap();

        let _ = execute_request(api_path, method.clone(), payload.clone(), Some(token_details.token.unwrap()), StatusCode::UNAUTHORIZED, &app).await;

        // --- Perform API call with token of non-verified user - Should fail ---

        let token_details = token::generate_jwt_token(jane().id, state.jwt.max_age, state.jwt.private_key.to_owned()).unwrap();

        let _ = execute_request(api_path, method.clone(), payload.clone(), Some(token_details.token.unwrap()), StatusCode::UNAUTHORIZED, &app).await;
        
        // --- Perform API call with user that logged out - Should fail ---

        let token = login(&john().email, &john().password, &app).await;
        
        let _ = execute_request("/auth/logout", Method::GET,
                                None::<Value>, Some(token.clone()),
                                StatusCode::OK, &app).await;
        
        let _ = execute_request(api_path, method.clone(), payload.clone(), Some(token), StatusCode::UNAUTHORIZED, &app).await;
    }
}