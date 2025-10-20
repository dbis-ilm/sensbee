use crate::{
    database::models::{
        data_chain::{DataChain, DataChainInternal, DataChainOutbound},
        events::signal_handler_change,
    },
    utils::AppError,
};
use sqlx::{PgPool, Row};
use tracing::{debug, error};
use uuid::Uuid;

pub async fn load(sensor_id: Uuid, db: &PgPool) -> anyhow::Result<Option<DataChain>> {
    let inbound = load_inbound(sensor_id, db).await?;

    match  sqlx::query_as::<_, DataChainOutbound>(
        "SELECT data_transformer_id, event_handler_id FROM sensor_data_chain_outbound WHERE sensor_id = $1",
    )
    .bind(sensor_id)
    .fetch_all(db)
    .await {
        Ok(v) => {
            return Ok(Some(DataChain{
                inbound,
                outbound: Some(v),
            }));
        },
        Err(err) => error!("{}", err),
    };
    Ok(None)
}

pub async fn load_inbound(sensor_id: Uuid, db: &PgPool) -> anyhow::Result<Option<Uuid>> {
    let row = sqlx::query("SELECT * FROM sensor_data_chain WHERE sensor_id = $1")
        .bind(sensor_id)
        .fetch_optional(db)
        .await?;
    match row {
        Some(r) => Ok(Some(r.try_get("inbound_dt_id")?)),
        None => Ok(None),
    }
}

/// Retrieves all event_handler that are used in outbound data chains.
pub async fn get_sensor_event_handler(
    db: &PgPool,
) -> anyhow::Result<Option<Vec<DataChainInternal>>> {
    let res = sqlx::query(
        "SELECT sensor_id, data_transformer_id, event_handler_id FROM sensor_data_chain_outbound",
    )
    .fetch_all(db)
    .await?;

    if res.is_empty() {
        Ok(None)
    } else {
        let mut chains = Vec::with_capacity(res.len());
        for row in res {
            chains.push(DataChainInternal {
                sensor_id: row.try_get("sensor_id")?,
                event_handler_id: row.try_get("event_handler_id")?,
                data_transformer_id: row.try_get("data_transformer_id")?,
            });
        }
        Ok(Some(chains))
    }
}

pub async fn set(sensor_id: Uuid, chain: &DataChain, db: &PgPool) -> anyhow::Result<(), AppError> {
    let mut tx = db.begin().await.unwrap();

    debug!("setting data chain of {} to {:?}", sensor_id, chain);

    // Remove old chains if they existed
    let _ = delete(sensor_id, db).await;

    if let Some(inbound) = chain.inbound {
        sqlx::query("INSERT INTO sensor_data_chain(sensor_id, inbound_dt_id) VALUES($1, $2)")
            .bind(sensor_id)
            .bind(inbound)
            .execute(&mut *tx)
            .await?;
    }

    if let Some(outbounds) = &chain.outbound {
        for e in outbounds {
            sqlx::query("INSERT INTO sensor_data_chain_outbound(sensor_id, data_transformer_id, event_handler_id) VALUES($1, $2, $3)")
            .bind(sensor_id)
                .bind(e.data_transformer_id)
                .bind(e.event_handler_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    // TODO should be part of tx
    let _ = signal_handler_change(db).await;

    let _ = tx.commit().await;

    Ok(())
}

pub async fn delete(sensor_id: Uuid, db: &PgPool) -> anyhow::Result<(), AppError> {
    let mut tx = db.begin().await.unwrap();

    let _res = sqlx::query("DELETE FROM sensor_data_chain WHERE sensor_id = $1")
        .bind(sensor_id)
        .execute(&mut *tx)
        .await?;
    let _res2 = sqlx::query("DELETE FROM sensor_data_chain_outbound WHERE sensor_id = $1")
        .bind(sensor_id)
        .execute(&mut *tx)
        .await?;

    // TODO should be part of tx
    let _ = signal_handler_change(db).await;

    let _ = tx.commit().await;

    Ok(())
}
