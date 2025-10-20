use crate::{
    database::models::events::{signal_handler_change, EventHandler},
    handler::models::{requests::CreateEventHandlerRequest, responses::GenericUuidResponse},
    utils::AppError,
};
use sqlx::PgPool;

/* ------------------------------------------------ Data transforms ------------------------------------------------------------ */

///
/// List/Load functions
///

/// Get a list of all available data transformer
/// NOTE this does not load the script content. Only ID and Name of each element.
pub async fn list(db: &PgPool) -> anyhow::Result<Vec<EventHandler>> {
    let res = sqlx::query_as::<_, EventHandler>("SELECT id, name FROM event_handler")
        .fetch_all(db)
        .await?;

    Ok(res)
}

// TODO should take &mut and load the script into the struct instead?
/// Load the complete data transformer entry associated with the given id.
pub async fn load(id: uuid::Uuid, db: &PgPool) -> anyhow::Result<EventHandler, AppError> {
    let res = sqlx::query_as::<_, EventHandler>("SELECT * FROM event_handler WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await?;
    if res.is_none() {
        return Err(AppError::not_found2(format!(
            "event_handler {} not found",
            id
        )));
    }

    return Ok(res.unwrap());
}

///
/// Create/Update/Delete functions
///

pub async fn create(
    req: CreateEventHandlerRequest,
    db: &PgPool,
) -> anyhow::Result<GenericUuidResponse> {
    let handler = req.to_new_handler();

    let _ =
        sqlx::query("INSERT INTO event_handler(id,name,filter,url,method) VALUES($1,$2,$3,$4,$5)")
            .bind(handler.id)
            .bind(handler.name)
            .bind(handler.filter)
            .bind(handler.url)
            .bind(handler.method)
            .execute(db)
            .await?;

    Ok(GenericUuidResponse {
        uuid: handler.id.to_string(),
    })
}

pub async fn delete(id: uuid::Uuid, db: &PgPool) -> anyhow::Result<(), AppError> {
    let mut tx = db.begin().await?;

    // Delete outbound chains that use this event handler
    let _ = sqlx::query("DELETE FROM sensor_data_chain_outbound WHERE event_handler_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // remove the event_handler
    let rows = sqlx::query("DELETE FROM event_handler WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if rows.rows_affected() == 0 {
        let _ = tx.rollback().await;
        return AppError::not_found(format!("event_handler with uuid {} does not exist", id));
    }

    let _ = tx.commit().await.unwrap();

    let _ = signal_handler_change(db).await?;

    Ok(())
}
