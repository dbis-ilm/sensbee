use std::sync::Arc;

use crate::database::models::db_structs::DBOrdering;
use crate::database::models::sensor::{ColumnType, FullSensorInfo, SensorColumn};
use crate::features::cache;
use crate::features::config::TIMESTAMP_FORMAT;
use crate::handler::models::requests::{
    DataLoadRequestParams, SensorDataDeletionParams, SensorDataIngestEntry,
};
use crate::state::AppState;
use serde_json::{Map, Value};
use sqlx::{Execute, Postgres, QueryBuilder, Row};

pub const TIME_COL_NAME: &str = "created_at";
pub const GROUPED_TIME_COL_NAME: &str = "grouped_time";

/// Deletes entries from the sensor data table in the specified time range.
pub async fn delete_sensor_data(
    sensor_id: uuid::Uuid,
    data: SensorDataDeletionParams,
    state: &AppState,
) -> anyhow::Result<()> {
    let sensor = cache::request_sensor(sensor_id, &state).await;

    if sensor.is_none() {
        anyhow::bail!("Sensor with id {} not found!", sensor_id);
    }

    let sensor = sensor.unwrap();

    let mut query_builder: QueryBuilder<Postgres> =
        QueryBuilder::new(format!("DELETE FROM {}", sensor.tbl_name));

    query_builder.push(create_timestamp_range_predicate(data.from, data.to, 
                                                        data.from_inclusive.unwrap_or(true),
                                                        data.to_inclusive.unwrap_or(true)));

    let query = query_builder.build();

    let query_result = query
        .execute(&state.db)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if query_result.is_err() {
        println!("{:?}", query_result.unwrap_err());
        anyhow::bail!("Couldn't delete sensor data in specified range!");
    }

    Ok(())
}

/// Fetches data specified by the given predicates in SensorDataRequest.
pub async fn get_data(
    sensor_id: uuid::Uuid,
    request: DataLoadRequestParams,
    state: &AppState,
) -> anyhow::Result<Value> {
    let sensor = cache::request_sensor(sensor_id, &state).await;

    if sensor.is_none() {
        anyhow::bail!("Sensor with id {} not found!", sensor_id);
    }

    let sensor = sensor.unwrap();

    // --- Create Select Section ---

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT ");
    let mut separated = query_builder.separated(", ");

    let mut requested_cols: Vec<SensorColumn> = Vec::new();

    match &request.cols {
        Some(cols) => {
            // Only consider specified data columns / time column

            let mut agg_cols = 0;
            let mut non_agg_cols = 0;

            for col in cols.iter() {
                if col.name == TIME_COL_NAME {
                    // Skip for now, added later
                    continue;
                }

                let sensor_col = sensor.columns.iter().find(|c| c.name == col.name);

                if sensor_col.is_none() {
                    anyhow::bail!("Data column {} does not exist!", col.name);
                }

                match &col.aggregation {
                    Some(aggregation) => {
                        let mut res_sens_col = sensor_col.unwrap().clone();
                        let target_type = aggregation.get_result_type(res_sens_col.val_type);

                        if target_type == ColumnType::UNKNOWN {
                            anyhow::bail!(
                                "Aggregation {} on column {} with type {:?} not allowed",
                                aggregation,
                                col.name,
                                res_sens_col.val_type
                            );
                        }

                        res_sens_col.val_type = target_type;

                        // Ensure correct type handling since aggregations return huge data types
                        let type_cast = format!("::{}", res_sens_col.val_type.to_sql_type());

                        agg_cols += 1;
                        separated.push(format!(
                            "{}({}){} as {}",
                            aggregation.as_db_op(),
                            col.name,
                            type_cast,
                            col.name
                        ));

                        requested_cols.push(res_sens_col);
                    }
                    None => {
                        non_agg_cols += 1;
                        separated.push(col.name.clone());
                        requested_cols.push(sensor_col.unwrap().to_owned());
                    }
                };
            }

            if agg_cols > 0 && request.time_grouping.is_none() {
                anyhow::bail!("Time grouping is missing for aggregated data retrieval!");
            }

            if request.time_grouping.is_some() && agg_cols == 0 {
                anyhow::bail!(
                    "Time grouping was specified but no aggregated data columns where provided!"
                );
            }

            if agg_cols > 0 && non_agg_cols > 0 {
                anyhow::bail!(
                    "Can't combine aggregated data columns with non aggregated data columns!"
                );
            }

            if let Some(time_grouping) = &request.time_grouping {
                // Modulo-like operation to create time buckets based on grouping interval
                separated.push(format!(
                    "to_timestamp(floor(extract(epoch FROM {}) / {}) * {})::timestamp AS {}",
                    TIME_COL_NAME, time_grouping, time_grouping, GROUPED_TIME_COL_NAME
                ));
            } else {
                separated.push(TIME_COL_NAME);
            }
        }
        None => {
            if request.time_grouping.is_some() {
                anyhow::bail!(
                    "Time grouping was specified but no aggregated data columns where provided!"
                );
            }

            // Default, include all data columns + time column
            separated.push(TIME_COL_NAME);

            for column in sensor.columns.as_slice() {
                separated.push(column.name.clone());

                requested_cols.push(column.to_owned());
            }
        }
    }

    if requested_cols.len() == 0 {
        anyhow::bail!("No data columns specified to retrieve!");
    }

    // --- Create From Section ---

    query_builder.push(" FROM ");
    query_builder.push(sensor.tbl_name.clone());

    // --- Handle predicates based on the request ---

    query_builder.push(create_timestamp_range_predicate(request.from, request.to, 
                                                        request.from_inclusive.unwrap_or(true), 
                                                        request.to_inclusive.unwrap_or(true)));

    // --- Handle grouping ---

    if request.time_grouping.is_some() {
        query_builder.push(format!(" GROUP BY {}", GROUPED_TIME_COL_NAME));
    }

    // --- Handle ordering and limit ---

    if request.ordering.is_some() {
        let mut order_col = request.order_col.unwrap_or(TIME_COL_NAME.to_string());

        // For aggregated queries with use the aggregated time col
        if request.time_grouping.is_some() && order_col == TIME_COL_NAME {
            order_col = GROUPED_TIME_COL_NAME.to_string();
        }

        match request.ordering.unwrap() {
            DBOrdering::ASC => query_builder.push(format!(" ORDER BY {} ASC", order_col)),
            DBOrdering::DESC => query_builder.push(format!(" ORDER BY {} DESC", order_col)),
            DBOrdering::DEFAULT => query_builder.push(""),
        };
    }

    if request.limit.is_some() {
        query_builder.push(format!(" LIMIT {}", request.limit.unwrap()));
    }

    // --- Build and execute query ---

    let query = query_builder.build();

    let sql = query.sql();
    log::debug!("query: {}", sql);

    let query_result = query
        .fetch_all(&state.db)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if query_result.is_err() {
        println!("{:?}", query_result.unwrap_err());
        anyhow::bail!("Couldn't fetch sensor data with the specified predicates!");
    }

    // --- Parse result ---

    let query_result = query_result.unwrap();

    let mut array = Vec::<serde_json::Value>::new();
    for row in query_result {
        let mut map = Map::new();

        if let Ok(created_at) = row.try_get::<chrono::NaiveDateTime, _>(TIME_COL_NAME) {
            map.insert(
                TIME_COL_NAME.to_string(),
                serde_json::json!(created_at.format(TIMESTAMP_FORMAT).to_string()),
            );
        }

        if let Ok(interval) = row.try_get::<chrono::NaiveDateTime, _>(GROUPED_TIME_COL_NAME) {
            map.insert(
                GROUPED_TIME_COL_NAME.to_string(),
                serde_json::json!(interval.format(TIMESTAMP_FORMAT).to_string()),
            );
        }

        // Extracts values for each requested column, inserting None if columns contains NULL or value is not parsable

        for col in requested_cols.as_slice() {
            let col_name = col.name.to_lowercase();

            match &col.val_type {
                ColumnType::INT => match row.try_get::<Option<i32>, _>(col_name.as_str()) {
                    Ok(val) => {
                        map.insert(col_name.clone(), serde_json::json!(val));
                    }
                    Err(err) => {
                        eprintln!(
                            "Error retrieving INT value for column '{}': {}",
                            col_name, err
                        );
                    }
                },

                ColumnType::FLOAT => match row.try_get::<Option<f64>, _>(col_name.as_str()) {
                    Ok(val) => {
                        map.insert(col_name.clone(), serde_json::json!(val));
                    }
                    Err(err) => {
                        eprintln!(
                            "Error retrieving FLOAT value for column '{}': {}",
                            col_name, err
                        );
                    }
                },

                ColumnType::STRING => match row.try_get::<Option<String>, _>(col_name.as_str()) {
                    Ok(val) => {
                        map.insert(col_name.clone(), serde_json::json!(val));
                    }
                    Err(err) => {
                        eprintln!(
                            "Error retrieving STRING value for column '{}': {}",
                            col_name, err
                        );
                    }
                },
                _ => {}
            }
        }

        array.push(serde_json::json!(map));
    }

    Ok(serde_json::json!(array))
}

fn create_timestamp_range_predicate(
    from: Option<chrono::NaiveDateTime>,
    to: Option<chrono::NaiveDateTime>,
    from_inclusive: bool,
    to_inclusive: bool,
) -> String {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("");

    let mut predicates: Vec<String> = Vec::new();

    if from.is_some() {
        predicates.push(format!(
            "created_at {} {}",
            if from_inclusive { ">=" } else { ">" },
            from.unwrap()
                .format(&format!("'{}'", TIMESTAMP_FORMAT).to_string())
        ));
    }

    if to.is_some() {
        predicates.push(format!(
            "created_at {} {}",
            if to_inclusive { "<=" } else { "<" },
            to.unwrap()
                .format(&format!("'{}'", TIMESTAMP_FORMAT).to_string())
        ));
    }

    if !predicates.is_empty() {
        query_builder.push(" WHERE ");

        for (idx, predicate) in predicates.iter().enumerate() {
            if idx != 0 {
                query_builder.push(" AND ");
            }

            query_builder.push(predicate);
        }
    }

    query_builder.into_sql()
}

/// Add sensor measurement data given as JSON data to the sensor data table.
/// The sensor is identified by the 'sensor_id' field, the columns and their values
/// are also given as fields the JSON object, e.g. "column1": value.
///
/// Insert the sensor data given by the JSON object to the given table.
/// If any of the entries to insert does not contain any valid values, the whole request will be rejected.
pub async fn add_sensor_data(
    sensor: Arc<FullSensorInfo>,
    data: &Vec<SensorDataIngestEntry>,
    state: AppState,
) -> anyhow::Result<()> {
    // INSERT INTO sensor.tbl_name () VALUES ()
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO ");
    query_builder.push(sensor.tbl_name.clone() + " (");

    // Push all columns of the sensor to query
    let mut cols_sep = query_builder.separated(", ");

    for col in sensor.columns.iter() {
        cols_sep.push(&col.name);
    }

    // If data provides explicit timestamp, otherwise auto-generated by the db
    cols_sep.push("created_at".to_string());

    cols_sep.push_unseparated(") VALUES");

    // Collect data to insert into sensor table

    for (idx, entry) in data.iter().enumerate() {
        let mut vals_sep = query_builder.separated(", ");

        if idx > 0 {
            vals_sep.push_unseparated(",");
        }

        // Only insert data for valid cols into the sensor, otherwise NULL

        let mut valid_cols = 0;

        vals_sep.push_unseparated(" (");

        for col in sensor.columns.iter() {
            let prov_col_val = &entry.data.get(&col.name);

            if prov_col_val.is_some() {
                valid_cols += 1;

                // Column is valid, parse provided value or NULl if invalid
                match &col.val_type {
                    ColumnType::INT => {
                        vals_sep.push_bind(prov_col_val.unwrap().as_i64());
                    }
                    ColumnType::FLOAT => {
                        vals_sep.push_bind(prov_col_val.unwrap().as_f64());
                    }
                    ColumnType::STRING => {
                        vals_sep.push_bind(prov_col_val.unwrap().as_str());
                    }
                    _ => {}
                }
            } else {
                match &col.val_type {
                    ColumnType::INT => {
                        vals_sep.push_bind(None::<i64>);
                    }
                    ColumnType::FLOAT => {
                        vals_sep.push_bind(None::<f64>);
                    }
                    ColumnType::STRING => {
                        vals_sep.push_bind(None::<String>);
                    }
                    _ => {}
                }
            }
        }

        // Insert created_at timestamp if provided in data - otherwise DEFAULT

        vals_sep.push(
            entry
                .timestamp
                .map(|ts| format!("'{}'", ts.format(TIMESTAMP_FORMAT)))
                .unwrap_or_else(|| "DEFAULT".to_string()),
        );

        vals_sep.push_unseparated(")");

        if valid_cols == 0 {
            anyhow::bail!(
                "No valid columns to insert data into the sensor {}!",
                sensor.id
            );
        }
    }

    // Execute query

    let query = query_builder.build();

    let mut tx = state.db.begin().await?;

    let res = query
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = res {
        anyhow::bail!(err)
    }

    let _ = tx.commit().await;

    Ok(())
}
