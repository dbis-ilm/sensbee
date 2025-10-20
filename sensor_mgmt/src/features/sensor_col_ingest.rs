use crate::database::models::sensor::ColumnIngest;
use anyhow::Result;
use sqlx::PgConnection;

// ------------------------------------------ Initialize -------------------------------------------

pub async fn register_sensor_col_ingest(
    ingest_mode: ColumnIngest,
    table_name: String,
    col_name: String,
    ex: &mut PgConnection,
) -> Result<()> {
    match ingest_mode {
        ColumnIngest::LITERAL => Ok(()),
        ColumnIngest::INCREMENTAL => on_register_incremental(table_name, col_name, ex).await,
    }
}

// Currently, we do not support modification of data columns (only call this function on sensor deletion)
pub async fn unregister_sensor_col_ingest(
    ingest_mode: ColumnIngest,
    table_name: String,
    col_name: String,
    ex: &mut PgConnection,
) -> Result<()> {
    match ingest_mode {
        ColumnIngest::LITERAL => Ok(()),
        ColumnIngest::INCREMENTAL => on_remove_incremental(table_name, col_name, ex).await,
    }
}

// ------------------------------------------ Incremental ------------------------------------------

async fn on_register_incremental(
    table_name: String,
    col_name: String,
    ex: &mut PgConnection,
) -> Result<()> {
    let res = sqlx::query(
        format!(
            "select create_sensor_column_ingest_incremental('{}', '{}');",
            table_name, col_name
        )
        .as_str(),
    )
    .execute(&mut *ex)
    .await
    .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = res {
        println!("Failed to initialize IncrementalIngestMode!");
        anyhow::bail!(err)
    }

    Ok(())
}

async fn on_remove_incremental(
    table_name: String,
    col_name: String,
    ex: &mut PgConnection,
) -> Result<()> {
    let res = sqlx::query(
        format!(
            "select remove_sensor_column_ingest_incremental('{}', '{}');",
            table_name, col_name
        )
        .as_str(),
    )
    .execute(&mut *ex)
    .await
    .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = res {
        println!("Failed to remove IncrementalIngestMode!");
        anyhow::bail!(err)
    }

    Ok(())
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use crate::database::data_db;
    use crate::database::models::db_structs::DBOperation;
    use crate::database::models::role::ROLE_SYSTEM_GUEST;
    use crate::database::models::sensor::{ColumnIngest, ColumnType, SensorColumn};
    use crate::features::cache;
    use crate::features::sensor_data_storage::{SensorDataStorageCfg, SensorDataStorageType};
    use crate::handler::models::requests::{
        CreateSensorRequest, DataLoadRequestParams, SensorDataIngestEntry, SensorPermissionRequest,
    };
    use crate::handler::models::responses::GenericUuidResponse;
    use crate::state::AppState;
    use crate::test_utils::tests::{
        add_dummy_data, create_test_app, create_test_sensors, execute_request, john, login,
    };
    use actix_http::{Method, StatusCode};
    use async_std::task;
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn add_custom_dummy_data(
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
                "col4": index,
            });

            let entry = SensorDataIngestEntry::from_json(payload.clone(), None);

            let sensor = Arc::new(cache::request_sensor(sensor_id, state).await.unwrap());

            data_db::add_sensor_data(sensor, &vec![entry], state.clone())
                .await
                .unwrap();

            task::sleep(core::time::Duration::from_millis(sleep_ms)).await;

            payloads.push(payload);
        }

        payloads
    }

    #[sqlx::test(
        migrations = "../migrations",
        fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql")
    )]
    async fn test_literal(pool: PgPool) {
        let (_, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let sensor_id = test_sens
            .iter()
            .find(|(name, _)| name == "MySensor5")
            .unwrap()
            .1;

        // Validate retrieval from db

        let sensor = cache::request_sensor(sensor_id, &state).await.unwrap();

        for elm in sensor.columns.iter() {
            assert_eq!(elm.val_ingest, ColumnIngest::LITERAL);
        }

        add_dummy_data(10, 0, 0, sensor_id, &state).await;

        // Check if all data remains the same as ingested (literal)

        let data = data_db::get_data(sensor_id, DataLoadRequestParams::default(), &state)
            .await
            .unwrap();
        let elms = data.as_array().unwrap();
        assert_eq!(elms.len(), 10);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col1")
                    .unwrap()
                    .as_i64()
                    .unwrap(),
                0 + index as i64
            );
        }
    }

    #[sqlx::test(
        migrations = "../migrations",
        fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql")
    )]
    async fn test_incremental(pool: PgPool) {
        let (app, state) = create_test_app(pool).await;

        let token = login(&john(), &state).await;

        // Create new sensor with incremental column

        let sensor_created = CreateSensorRequest {
            name: "MyIncrementalSensor".to_string(),
            description: Some("My new sensor!".to_string()),
            position: Some((50.0, 10.0)),
            permissions: vec![SensorPermissionRequest {
                role_id: ROLE_SYSTEM_GUEST,
                operations: DBOperation::all(),
            }],
            columns: vec![
                SensorColumn {
                    name: "col1".to_string(),
                    val_type: ColumnType::INT,
                    val_unit: "unit_1".to_string(),
                    val_ingest: ColumnIngest::INCREMENTAL,
                },
                SensorColumn {
                    name: "col2".to_string(),
                    val_type: ColumnType::FLOAT,
                    val_unit: "unit_2".to_string(),
                    val_ingest: ColumnIngest::INCREMENTAL,
                },
                SensorColumn {
                    name: "col3".to_string(),
                    val_type: ColumnType::STRING,
                    val_unit: "unit_3".to_string(),
                    val_ingest: ColumnIngest::INCREMENTAL,
                },
                SensorColumn {
                    name: "col4".to_string(),
                    val_type: ColumnType::INT,
                    val_unit: "unit_4".to_string(),
                    val_ingest: ColumnIngest::LITERAL,
                },
            ],
            storage: SensorDataStorageCfg {
                variant: SensorDataStorageType::Default,
                params: None,
            },
        };

        let body = execute_request(
            "/api/sensors/create",
            Method::POST,
            None,
            Some(sensor_created),
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;

        let resp: GenericUuidResponse = serde_json::from_value(body).unwrap();
        let sensor_id = Uuid::parse_str(&resp.uuid).unwrap();

        // Validate retrieval from db

        let sensor = cache::request_sensor(sensor_id, &state).await.unwrap();

        for (index, elm) in sensor.columns.iter().enumerate() {
            if index != 3 {
                assert_eq!(elm.val_ingest, ColumnIngest::INCREMENTAL);
            } else {
                assert_eq!(elm.val_ingest, ColumnIngest::LITERAL);
            }
        }

        // Ingest data into sensor

        let payloads = add_custom_dummy_data(6, 0, 10, sensor_id, &state).await;

        // Check if all data is incremented correctly

        let data = data_db::get_data(sensor_id, DataLoadRequestParams::default(), &state)
            .await
            .unwrap();
        let elms = data.as_array().unwrap();

        assert_eq!(elms.len(), 6);

        for (index, elm) in elms.iter().enumerate() {
            let mut sum_col1 = 0;
            let mut sum_col2 = 0.0;
            let mut sum_col3 = String::new();

            for i in 0..=index {
                let obj = payloads[i].as_object().unwrap();

                sum_col1 += obj.get("col1").unwrap().as_i64().unwrap();
                sum_col2 += obj.get("col2").unwrap().as_f64().unwrap();
                sum_col3 += obj.get("col3").unwrap().as_str().unwrap();
            }

            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col1")
                    .unwrap()
                    .as_i64()
                    .unwrap(),
                sum_col1
            );
            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col2")
                    .unwrap()
                    .as_f64()
                    .unwrap(),
                sum_col2
            );
            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col3")
                    .unwrap()
                    .as_str()
                    .unwrap(),
                sum_col3
            );
            assert_eq!(
                elm.as_object()
                    .unwrap()
                    .get("col4")
                    .unwrap()
                    .as_i64()
                    .unwrap(),
                0 + index as i64
            ); // This should stay the same
        }

        // Add null and check if the last value is retained correctly [stay at last val]

        let entry = SensorDataIngestEntry::from_json(
            json!({
                    "col1": null,
                    "col2": null,
                    "col3": null,
                    "col4": 0,
            }),
            None,
        );

        let sensor = Arc::new(cache::request_sensor(sensor_id, &state).await.unwrap());

        data_db::add_sensor_data(sensor, &vec![entry], state.clone())
            .await
            .unwrap();

        let data = data_db::get_data(sensor_id, DataLoadRequestParams::default(), &state.clone())
            .await
            .unwrap();
        let elms = data.as_array().unwrap();

        assert_eq!(elms.len(), 7);

        assert_eq!(
            elms.last()
                .unwrap()
                .as_object()
                .unwrap()
                .get("col1")
                .unwrap()
                .as_i64()
                .unwrap(),
            elms[elms.len() - 2]
                .as_object()
                .unwrap()
                .get("col1")
                .unwrap()
                .as_i64()
                .unwrap()
        );
        assert_eq!(
            elms.last()
                .unwrap()
                .as_object()
                .unwrap()
                .get("col2")
                .unwrap()
                .as_f64()
                .unwrap(),
            elms[elms.len() - 2]
                .as_object()
                .unwrap()
                .get("col2")
                .unwrap()
                .as_f64()
                .unwrap()
        );
        assert_eq!(
            elms.last()
                .unwrap()
                .as_object()
                .unwrap()
                .get("col3")
                .unwrap()
                .as_str()
                .unwrap(),
            elms[elms.len() - 2]
                .as_object()
                .unwrap()
                .get("col3")
                .unwrap()
                .as_str()
                .unwrap()
        );

        // Remove sensor and check if function gets deleted correctly

        let _ = execute_request(
            &format!("/api/sensors/{}/delete", sensor_id),
            Method::DELETE,
            None,
            None::<Value>,
            Some(token.clone()),
            StatusCode::OK,
            &app,
        )
        .await;
    }
}
