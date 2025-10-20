use crate::authentication::jwt_auth;
use crate::database::models::events::EventHandler;
use crate::database::{event_handler_db};
use crate::handler::models::requests::CreateEventHandlerRequest;
use crate::handler::{main_hdl, policy};
use crate::state::AppState;
use crate::{
    utils::AppError,
};
use actix_web::{delete, get, post, web, Responder};
use actix_web::{HttpResponse};

/* ------------------------------------------------ Event handler -------------------------------------------------- */

const COMMON_TAG: &str = "Event Handler";

#[utoipa::path(
    get,
    path = "/api/event_handler/list",
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns a list of data transformer that the requesting user has access to.", body = Vec<EventHandler>),
        (status = 401, description = "Returns an unauthorized error if no valid token was provided."),
    ),
    security(("JWT" = [])),
)]
#[get("/event_handler/list")]
async fn list_event_handler_handler(
    state: web::Data<AppState>,
    jwt: jwt_auth::JwtMiddleware,
) -> HttpResponse {
    let user_id = jwt.user_id;

    let login_id = policy::require_login(user_id, &state).await.map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized2("must be logged in".to_string()).into();
    }

    let res = event_handler_db::list(&state.db).await; 

   main_hdl::send_result(&res)
}

#[utoipa::path(
    get,
    path = "/api/event_handler/{id}/load",
    params( ("id" = Uuid, Path, description = "The uuid of the sensor", example = json!(uuid::Uuid::new_v4().to_string()))),
    tag = COMMON_TAG,
    responses(
        (status = 200, description= "Return the data transform script for this sensor if any is set.", body = EventHandler),
        (status = 401, description = "Returns an unauthorized error if no valid token was provided."),
    ),
    security(("JWT" = [])),
)]
#[get("/event_handler/{id}/load")]
async fn load_event_handler_handler(path: web::Path<uuid::Uuid>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> HttpResponse {

    let user_id = jwt.user_id;
    let transformer_id = path.into_inner();

    let login_id = policy::require_login(user_id, &state).await.map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized2("must be logged in".to_string()).into();
    }

    let res = event_handler_db::load(transformer_id, &state.db.clone()).await;
    if res.is_err() {
        return res.err().unwrap().into();
    }

    main_hdl::send_result(&Ok(res.ok().unwrap()))
}

#[utoipa::path(
    post,
    path = "/api/event_handler/create",
    request_body(
        content_type = "application/json",
        content = CreateEventHandlerRequest,
        description = "TODO",
        example = json!({"todo":"todo"}),
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns ok if the event_handler was created."),
        (status = 401, description= "Returns unauthorized if the request has no permissions to create an event_handler."),
        (status = 500, description= "Returns an error if the event_handler couldn't be created."),
    ),
    security(("JWT" = [])),
)]
#[post("/event_handler/create")]
async fn create_event_handler_handler(body: web::Json<CreateEventHandlerRequest>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {
    
    let user_id = jwt.user_id;
    let req = body.into_inner();

    // This requires login
    let login_id = policy::require_login(user_id, &state).await.map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized2("must be logged in").into();
    }

    let res = event_handler_db::create(req, &state.db).await;

    main_hdl::send_result(&res)
}

#[utoipa::path(
    delete,
    path = "/api/event_handler/{id}/delete",
    params( 
        ("id" = Uuid, Path, description = "The uuid of the event handler that should be removed.", example = json!(uuid::Uuid::new_v4().to_string()))
    ),
    tag = COMMON_TAG,
    responses(
        (status = 200, description = "Returns ok if the data transformation script was deleted."),
        (status = 401, description= "Returns an unauthorized error if the request has no permissions to delete the data transformation script for the sensor."),
        (status = 500, description= "Returns an error if the event_handler couldn't be deleted."),
    ),
    security(("JWT" = [])),
)]

#[delete("/event_handler/{id}/delete")]
async fn delete_event_handler_handler(path: web::Path<uuid::Uuid>, state: web::Data<AppState>, jwt: jwt_auth::JwtMiddleware) -> impl Responder {

    let user_id = jwt.user_id;
    let event_handler_id = path.into_inner();

    let login_id = policy::require_login(user_id, &state).await.map_or(user_id, |_| None);
    if login_id.is_none() {
        return AppError::unauthorized("must be logged in".to_string());
    }

    event_handler_db::delete(event_handler_id, &state.db).await
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{handler::models::responses::GenericUuidResponse, test_utils::tests::{
        create_test_app, execute_request, john, login, test_invalid_auth,
    }};
    use actix_http::Method;
    use actix_web::http::StatusCode;
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use std::str::FromStr;
    use uuid::Uuid;

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_list_event_handler(pool: PgPool){
        // Setup
        let (app, state) = create_test_app(pool).await;

        // -- list without login -- should fail
        test_invalid_auth(
            format!("/api/event_handler/list").as_str(),
            Method::GET,
            None::<Value>,
            &state,
            &app,
        )
        .await;

        let token = login(&john(), &state).await;

        // -- list without any scripts existing -- should return empty

        let res = execute_request(
            &format!("/api/event_handler/list"),
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
            CreateEventHandlerRequest {
                name: "TheHandler1".to_string(),
                filter: "1Filter".to_string(),
                url: "1Url".to_string(),
                method: "1Method".to_string(),
            },
            CreateEventHandlerRequest {
                name: "TheHandler2".to_string(),
                filter: "2Filter".to_string(),
                url: "2Url".to_string(),
                method: "2Method".to_string(),
            },
            CreateEventHandlerRequest {
                name: "TheHandler3".to_string(),
                filter: "3Filter".to_string(),
                url: "3Url".to_string(),
                method: "3Method".to_string(),
            },
        ];
        let mut created_transformer_uuids = vec![];
        for t in transformer.clone() {
            let res = execute_request(
                &format!("/api/event_handler/create"),
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
            &format!("/api/event_handler/list"),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        assert!(json!([]) != res);

        // validate that the created event_handler have the correct name for theirs IDs
        let r: Vec<EventHandler> = res
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
    async fn test_load_event_handler(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;
        
        let payload = CreateEventHandlerRequest {
            name: "A script".to_string(),
            filter: "the filter".to_string(),
            url: "the url".to_string(),
            method: "the method".to_string(),
        };

        test_invalid_auth(
            format!("/api/event_handler/create").as_str(),
            Method::POST,
            Some(payload.clone()),
            &state,
            &app,
        )
        .await;

        let token = login(&john(), &state).await;

        // --- Create an event_handler -- should work

        let res = execute_request(
            &format!("/api/event_handler/create"),
            Method::POST,
            None,
            Some(payload.clone()),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let resp_id: GenericUuidResponse = serde_json::from_value(res).unwrap();

        // --- Retrieve the saved event_handler and make sure it matches -- should work ---

        let body = execute_request(
            &format!("/api/event_handler/{}/load", resp_id.uuid),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let dt: EventHandler = serde_json::from_value(body).unwrap();
        // Assert that all fields are as expected
        assert!(payload.name == dt.name);
        assert!(payload.url == dt.url);
        assert!(payload.filter == dt.filter);
        assert!(payload.method == dt.method);

        // --- Retrieve an event_handler from a non-existent data_transform -- Should fail ---

        let _ = execute_request(
            &format!("/api/event_handler/{}/load", Uuid::new_v4()),
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
    async fn test_create_event_handler(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let payload = CreateEventHandlerRequest {
            name: "A handler".to_string(),
            method:"themethod".to_string(),
            url: "someurl".to_string(),
            filter: "filter".to_string(),
        };

        test_invalid_auth(
            format!("/api/event_handler/create").as_str(),
            Method::POST,
            Some(payload.clone()),
            &state,
            &app,
        )
        .await;

        // --- Create data transform script as john - Should succeed ---

        let token = login(&john(), &state).await;

        let created_resp = execute_request(
            &format!("/api/event_handler/create"),
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
            &format!("/api/event_handler/{}/load", created_id.uuid),
            Method::GET,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
        let load_resp: EventHandler = serde_json::from_value(res).unwrap();
        assert!(load_resp.name == payload.name);
        assert!(load_resp.method == payload.method);
        assert!(load_resp.url == payload.url);
        assert!(load_resp.filter == payload.filter);
    }

    #[sqlx::test(migrations = "../migrations", fixtures("users", "roles", "user_roles"))]
    async fn test_delete_event_handler(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let token = login(&john(), &state).await;

        let payload = CreateEventHandlerRequest {
            name: "A handler".to_string(),
            method:"themethod".to_string(),
            url: "someurl".to_string(),
            filter: "filter".to_string(),
        };

        let res = execute_request(
            &format!("/api/event_handler/create"),
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
            format!("/api/event_handler/{}/delete", resp.uuid).as_str(),
            Method::DELETE,
            Some(payload.clone()),
            &state,
            &app,
        )
        .await;

        let token = login(&john(), &state).await;

        // --- delete the data_transformer -- should work
        let _ = execute_request(
            &format!("/api/event_handler/{}/delete", resp.uuid),
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
            &format!("/api/event_handler/{}/delete", resp.uuid),
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
