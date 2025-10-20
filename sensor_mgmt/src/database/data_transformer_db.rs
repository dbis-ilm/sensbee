use crate::database::models::data_transformer::DataTransformer;
use crate::database::models::events::signal_handler_change;
use crate::handler::models::requests::{
    CreateDataTransformScriptRequest, UpdateDataTransformScriptRequest,
};
use crate::handler::models::responses::GenericUuidResponse;
use crate::utils::AppError;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/* ------------------------------------------------ Data transforms ------------------------------------------------------------ */

///
/// List/Load functions
///

/// Get a list of all available data transformer
/// NOTE this does not load the script content. Only ID and Name of each element.
pub async fn list(db: &PgPool) -> anyhow::Result<Vec<DataTransformer>> {
    let res = sqlx::query_as::<_, DataTransformer>(
        "SELECT id, name, created_at, updated_at, version FROM data_transformer",
    )
    .fetch_all(db)
    .await?;

    Ok(res)
}

/// Load the complete data transformer entry associated with the given id.
pub async fn load(id: uuid::Uuid, db: &PgPool) -> anyhow::Result<DataTransformer, AppError> {
    let res = sqlx::query_as::<_, DataTransformer>("SELECT * FROM data_transformer WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await?;
    if res.is_none() {
        return Err(AppError::not_found2(format!(
            "data_transformer {} not found",
            id
        )));
    }

    return Ok(res.unwrap());
}

///
/// Creation/Update/Delete functions
///

pub async fn create(
    req: CreateDataTransformScriptRequest,
    db: &PgPool,
) -> anyhow::Result<GenericUuidResponse> {
    // Validation
    if req.name.len() < 3 {
        //return AppError::db(format!("Validation failed: name must be at least 3 chars"));
    }

    // Create uuid
    let id = uuid::Uuid::new_v4();

    // insert into db and set foreign key on sensor
    let affected_rows =
        sqlx::query("INSERT INTO data_transformer(id, name, script, version) VALUES($1,$2,$3,1)")
            .bind(id)
            .bind(req.name)
            .bind(req.script)
            .execute(db)
            .await?;
    if affected_rows.rows_affected() != 1 {
        //return AppError::db(format!("creating transform_script failed"));
    }

    Ok(GenericUuidResponse {
        uuid: id.to_string(),
    })
}

pub async fn update(
    id: Uuid,
    req: &UpdateDataTransformScriptRequest,
    db: &PgPool,
) -> anyhow::Result<GenericUuidResponse> {
    // Validation
    if req.name.len() < 3 {
        //return AppError::db(format!("Validation failed: name must be at least 3 chars"));
    }

    let mut ts = load(id, db).await?;

    // A script id references a specific script text.
    // Therefore each script update generates a new id
    // NOTE the data transformer service depends on this behaviour
    let id = uuid::Uuid::new_v4();

    // Increase version number
    ts.version = ts.version + 1;
    ts.updated_at = Some(Utc::now().naive_utc());

    // update
    let affected_rows = sqlx::query("UPDATE data_transformer SET id = $2, name = $3, script = $4, version = $5, updated_at = $6 WHERE id = $1")
        .bind(ts.id)
        .bind(id)
        .bind(req.name.clone())
        .bind(req.script.clone())
        .bind(ts.version)
        .bind(ts.updated_at)
        .execute(db)
        .await?;
    if affected_rows.rows_affected() != 1 {
        //return AppError::db(format!("creating transform_script failed"));
    }

    // TODO should be part of tx
    let _ = signal_handler_change(db).await;

    Ok(GenericUuidResponse {
        uuid: id.to_string(),
    })
}

pub async fn delete(id: uuid::Uuid, db: &PgPool) -> anyhow::Result<(), AppError> {
    let rows = sqlx::query("DELETE FROM data_transformer WHERE id=$1")
        .bind(id)
        .execute(db)
        .await?;
    if rows.rows_affected() == 0 {
        return AppError::not_found(format!("data_transformer with uuid {} does not exist", id));
    }

    // TODO should be part of tx
    let _ = signal_handler_change(db).await;

    Ok(())
}
