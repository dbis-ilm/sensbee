use sqlx::{Error, PgPool};
use uuid::Uuid;

use crate::database::models::events::LogEventRow;

///
/// Functions to save/load sensor related events
///

pub async fn get_sensor_event_history(
    mut limit: i32,
    pool: PgPool,
    sensor_id: Uuid,
) -> Result<Vec<LogEventRow>, Error> {
    // Set a sensible limit
    if limit == 0 {
        limit = 100;
    }
    if limit > 1000 {
        limit = 1000;
    }

    sqlx::query_as::<_, LogEventRow>(
        r#"SELECT t AT TIME ZONE 'UTC' as t, sensor_id, data 
FROM log_events 
WHERE sensor_id = $1
ORDER by t ASC
LIMIT $2"#,
    )
    .bind(sensor_id)
    .bind(limit)
    .fetch_all(&pool)
    .await
}

pub async fn get_general_event_history(pool: PgPool) -> anyhow::Result<Vec<LogEventRow>> {
    let res = sqlx::query_as::<_, LogEventRow>(
        r#"SELECT t AT TIME ZONE 'UTC' as t, sensor_id, data 
FROM log_events 
WHERE sensor_id is null
ORDER by t ASC"#,
    )
    .fetch_all(&pool)
    .await?;

    Ok(res)
}
