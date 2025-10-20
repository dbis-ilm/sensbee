use crate::authentication::jwt_auth;
use crate::database::data_transformer_db::{self};
use crate::database::models::data_transformer::DataTransformer;
use crate::handler::models::requests::{
    CreateDataTransformScriptRequest, UpdateDataTransformScriptRequest,
};
use crate::handler::models::responses::GenericUuidResponse;
use crate::handler::{main_hdl, policy};
use crate::state::AppState;
use crate::utils::AppError;
use actix_web::{delete, get, post, web, HttpResponse, Responder};

/* ------------------------------------------------ Data Transformer -------------------------------------------------- */

const COMMON_TAG: &str = "Data Transformer";

#[utoipa::path(
    get,
    path = "/api/data_transformer/list",
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns a list of data transformer that the requesting user has access to. Not all fields contain values. Use load for details.", body = Vec<DataTransformer>),
        (status = 401, description = "Returns an unauthorized error if no valid token was provided."),
    ),
    security(("JWT" = [])),
)]
#[get("/data_transformer/list")]
async fn list_data_transformer_handler(
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> HttpResponse {
    let user_id = jwt.user_id;

    let login_id = policy::require_login(user_id, &state)
        .await
        .map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized("must be logged in".to_string())
            .err()
            .unwrap()
            .into();
    }

    let res = data_transformer_db::list(&state.db).await;

    main_hdl::send_result(&res)
}

#[utoipa::path(
    get,
    path = "/api/data_transformer/{id}/load",
    params( ("id" = String, Path, description = "The uuid of the data_transformer.", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = COMMON_TAG,
    responses(
        (status = 200, description= "Returns the data_transformer info for the requested uuid if it exists.", body = DataTransformer),
        (status = 401, description = "Returns an unauthorized error if no valid token was provided."),
    ),
    security(("JWT" = [])),
)]
#[get("/data_transformer/{id}/load")]
async fn load_data_transformer_handler(
    path: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> HttpResponse {
    let user_id = jwt.user_id;

    let login_id = policy::require_login(user_id, &state)
        .await
        .map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized("must be logged in".to_string())
            .err()
            .unwrap()
            .into();
    }

    let res = data_transformer_db::load(path.into_inner(), &state.db.clone()).await;
    if res.is_err() {
        return res.err().unwrap().into();
    }

    main_hdl::send_result(&Ok(res.ok().unwrap()))
}

#[utoipa::path(
    post,
    path = "/api/data_transformer/create",
    request_body(
        content_type = "application/json",
        content = CreateDataTransformScriptRequest,
        description = "TODO",
        example = json!({"name":"the name","script":"return {\"a\":\"a value\"};"}),
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns OK if the data transformation script was created for the given sensor.", body = GenericUuidResponse),
        (status = 401, description= "Returns an unauthorized error if the request has no permissions to create a data transformation script."),
        (status = 500, description= "Returns an error if the data transformation script couldn't be created."),
    ),
    security(("JWT" = [])),
)]
#[post("/data_transformer/create")]
async fn create_data_transformer_handler(
    body: web::Json<CreateDataTransformScriptRequest>,
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> impl Responder {
    let user_id = jwt.user_id;
    let req = body.into_inner();

    // This requires login
    let login_id = policy::require_login(user_id, &state)
        .await
        .map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized("must be logged in".to_string())
            .err()
            .unwrap()
            .into();
    }

    let res = data_transformer_db::create(req, &state.db).await;

    main_hdl::send_result(&res)
}

#[utoipa::path(
    post,
    path = "/api/data_transformer/{id}/update",
    params(
        ("id" = String, Path, description = "The uuid of the data_transformer that should be updated.", example = json!(uuid::Uuid::new_v4().to_string()))
    ),
    request_body(
        content_type = "application/json",
        content = UpdateDataTransformScriptRequest,
        description = "TODO",
        example = json!({"name":"an updated name","script":"return {\"a\":\"a value\"};"}),
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns OK if the data transformation script was updated.",body = GenericUuidResponse),
        (status = 401, description= "Returns an unauthorized error if the request has no permissions to create a data transformation script."),
        (status = 500, description= "Returns an error if the data transformation script couldn't be created."),
    ),
    security(("JWT" = [])),
)]
#[post("/data_transformer/{id}/update")]
async fn update_data_transformer_handler(
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdateDataTransformScriptRequest>,
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> impl Responder {
    let user_id = jwt.user_id;
    let req = body.into_inner();

    // This requires login
    let login_id = policy::require_login(user_id, &state)
        .await
        .map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized("must be logged in".to_string())
            .err()
            .unwrap()
            .into();
    }

    let res = data_transformer_db::update(path.into_inner(), &req, &state.db).await;

    main_hdl::send_result(&res)
}

#[utoipa::path(
    delete,
    path = "/api/data_transformer/{id}/delete",
    params(
        ("id" = String, Path, description = "The uuid of the data_transformer that should be deleted.", example = json!(uuid::Uuid::new_v4().to_string()))
    ),
    tag = COMMON_TAG,
    responses(
        (status = 204, description = "Returns NO_CONTENT if the data transformation script was deleted."),
        (status = 401, description= "Returns an unauthorized error if the request has no permissions to delete the data transformation script for the sensor."),
        (status = 500, description= "Returns an error if the api key couldn't be deleted."),
    ),
    security(("JWT" = [])),
)]
#[delete("/data_transformer/{id}/delete")]
async fn delete_data_transformer_handler(
    path: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> impl Responder {
    let user_id = jwt.user_id;
    let data_transformer_id = path.into_inner();

    let login_id = policy::require_login(user_id, &state)
        .await
        .map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized("must be logged in".to_string());
    }

    data_transformer_db::delete(data_transformer_id, &state.db).await
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::test_utils::tests::{
        create_test_app, execute_request, john, login, test_invalid_auth,
    };
    use actix_http::Method;
    use actix_web::http::StatusCode;
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use std::str::FromStr;
    use uuid::Uuid;

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_list_transform_script(pool: PgPool) {
        // Setup
        let (app, state) = create_test_app(pool).await;

        // -- list without login -- should fail
        test_invalid_auth(
            format!("/api/data_transformer/list").as_str(),
            Method::GET,
            None::<Value>,
            &state,
            &app,
        )
        .await;

        let token = login(&john(), &state).await;

        // -- list without any scripts existing -- should return empty

        let res = execute_request(
            &format!("/api/data_transformer/list"),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        assert!(res == json!([]));

        // -- list with existing scripts -- should return all created
        // Create some scripts
        let transformer = vec![
            CreateDataTransformScriptRequest {
                name: "TheTransformer1".to_string(),
                script: "//This is some valid js\nreturn {};".to_owned(),
            },
            CreateDataTransformScriptRequest {
                name: "TheTransformer2".to_string(),
                script: "What even is this. Definitly not js.".to_owned(),
            },
            CreateDataTransformScriptRequest {
                name: "TheTransformer3".to_string(),
                script: "//This script uses emojis\n return {\"a\":\"👌\"};".to_owned(),
            },
        ];
        let mut created_transformer_uuids = vec![];
        for t in transformer.clone() {
            let res = execute_request(
                &format!("/api/data_transformer/create"),
                Method::POST,
                None,
                Some(t.clone()),
                Some(token.clone()),
                StatusCode::OK,
                &app,
            )
            .await;
            let parsed_res: GenericUuidResponse = serde_json::from_value(res).unwrap();
            created_transformer_uuids.push(Uuid::from_str(&parsed_res.uuid).unwrap());
        }
        assert!(created_transformer_uuids.len() == transformer.len());

        // List them again
        let res = execute_request(
            &format!("/api/data_transformer/list"),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        assert!(json!([]) != res);

        // validate that the created transformer have the correct name for theirs IDs
        let r: Vec<DataTransformer> = res
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| serde_json::from_value(entry.to_owned()).unwrap())
            .collect();
        for dt in r {
            assert!(created_transformer_uuids.contains(&dt.id));
        }
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_load_transform_script(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        // NOTE we dont syntax check the incoming script on our side
        // this test only checks if a value can be saved as a script. Syntax check is done by the transform service on script compile
        let payload = CreateDataTransformScriptRequest {
            name: "A script".to_string(),
            script: "A value".to_owned(),
        };

        test_invalid_auth(
            format!("/api/data_transformer/create").as_str(),
            Method::POST,
            Some(payload.clone()),
            &state,
            &app,
        )
        .await;

        let token = login(&john(), &state).await;

        // --- Create a script --- should work ---

        let res = execute_request(
            &format!("/api/data_transformer/create"),
            Method::POST,
            None,
            Some(payload.clone()),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let resp_id: GenericUuidResponse = serde_json::from_value(res).unwrap();

        // --- Retrieve the saved script and make sure it matches -- should work ---

        let body = execute_request(
            &format!("/api/data_transformer/{}/load", resp_id.uuid),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let dt: DataTransformer = serde_json::from_value(body).unwrap();
        assert!(payload.script == dt.script);
        assert!(payload.name == dt.name);

        // --- Retrieve a script from a non-existent data_transform -- Should fail ---

        let _ = execute_request(
            &format!("/api/data_transformer/{}/load", Uuid::new_v4()),
            Method::GET,
            None,
            Some(payload.clone()),
            Some(token.clone()),
            StatusCode::NOT_FOUND,
            &app,
        )
        .await;
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_create_data_transform_script(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        // NOTE we dont syntax check the incoming script on our side
        // this test only checks if a value can be saved as a script. Syntax check is done by the transform service on script compile
        let payload = CreateDataTransformScriptRequest {
            name: "A script".to_string(),
            script: "A value".to_owned(),
        };

        test_invalid_auth(
            format!("/api/data_transformer/create").as_str(),
            Method::POST,
            Some(payload.clone()),
            &state,
            &app,
        )
        .await;

        // --- Create data transform script as john - Should succeed ---

        let token = login(&john(), &state).await;

        let created_resp = execute_request(
            &format!("/api/data_transformer/create"),
            Method::POST,
            None,
            Some(payload.clone()),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let created_id: GenericUuidResponse = serde_json::from_value(created_resp).unwrap();

        // --- Make sure that the stored values are correct --- should work
        let res = execute_request(
            &format!("/api/data_transformer/{}/load", created_id.uuid),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let load_resp: DataTransformer = serde_json::from_value(res).unwrap();
        assert!(load_resp.name == payload.name);
        assert!(load_resp.script == payload.script);
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_update_data_transform_script(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let token = login(&john(), &state).await;

        let payload = CreateDataTransformScriptRequest {
            name: "A script".to_string(),
            script: "A value".to_owned(),
        };
        let res = execute_request(
            &format!("/api/data_transformer/create"),
            Method::POST,
            None,
            Some(payload.clone()),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let resp: GenericUuidResponse = serde_json::from_value(res).unwrap();

        // --- make sure that you need to logged int to interact with the endpoint -- should fail
        test_invalid_auth(
            format!("/api/data_transformer/{}/update", resp.uuid).as_str(),
            Method::POST,
            Some(payload.clone()),
            &state,
            &app,
        )
        .await;

        // --- Update the script -- should work
        let payload_updated = CreateDataTransformScriptRequest {
            name: "The updated script".to_string(),
            script: "A updated value".to_owned(),
        };
        let res = execute_request(
            &format!("/api/data_transformer/{}/update", resp.uuid),
            Method::POST,
            None,
            Some(payload_updated.clone()),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let new_id_resp: GenericUuidResponse = serde_json::from_value(res).unwrap();

        // --- Make sure that the stored values are correct --- should work
        let res = execute_request(
            &format!("/api/data_transformer/{}/load", new_id_resp.uuid),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let load_resp: DataTransformer = serde_json::from_value(res).unwrap();
        assert!(load_resp.name == payload_updated.name);
        assert!(load_resp.script == payload_updated.script);
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_data_transform_script(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let token = login(&john(), &state).await;

        let payload = CreateDataTransformScriptRequest {
            name: "A script".to_string(),
            script: "A value".to_owned(),
        };
        let res = execute_request(
            &format!("/api/data_transformer/create"),
            Method::POST,
            None,
            Some(payload.clone()),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let resp: GenericUuidResponse = serde_json::from_value(res).unwrap();

        // --- make sure that you need to be logged in to interact with the endpoint -- should fail
        test_invalid_auth(
            format!("/api/data_transformer/{}/delete", resp.uuid).as_str(),
            Method::DELETE,
            Some(payload.clone()),
            &state,
            &app,
        )
        .await;

        let token = login(&john(), &state).await;

        // --- delete the data_transformer -- should work
        let _ = execute_request(
            &format!("/api/data_transformer/{}/delete", resp.uuid),
            Method::DELETE,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::NO_CONTENT,
            &app,
        )
        .await;

        // --- delete the same data_transformer again -- should fail
        let _ = execute_request(
            &format!("/api/data_transformer/{}/delete", resp.uuid),
            Method::DELETE,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::NOT_FOUND,
            &app,
        )
        .await;
    }
}
