use serde_derive::{Deserialize, Serialize};
use utoipa::ToSchema;
use anyhow::Result;
use serde_json::{Map, Value};
use sqlx::PgConnection;

// ------------------------------------------ Initialize -------------------------------------------

const DEFAULT_OBJ: DefaultSDS = DefaultSDS;
const RING_BUFFER_COUNT_OBJ: RingBufferCountSDS = RingBufferCountSDS;
const RING_BUFFER_INTERVAL_OBJ: RingBufferIntervalSDS = RingBufferIntervalSDS;

// Workaround, since async traits are not fully supported right now! :(

pub async fn register_sensor_data_storage(storage_type: SensorDataStorageType, table_name: String, params: Option<Map<String, Value>>, ex: &mut PgConnection) -> Result<()> {
    match storage_type {
        SensorDataStorageType::Default => DEFAULT_OBJ.on_initialize(table_name, params, ex).await,
        SensorDataStorageType::RingBufferCount => RING_BUFFER_COUNT_OBJ.on_initialize(table_name, params, ex).await,
        SensorDataStorageType::RingBufferInterval => RING_BUFFER_INTERVAL_OBJ.on_initialize(table_name, params, ex).await,
    }
}

pub async fn unregister_sensor_data_storage(storage_type: SensorDataStorageType, table_name: String, ex: &mut PgConnection) -> Result<()> {
    match storage_type {
        SensorDataStorageType::Default => DEFAULT_OBJ.on_remove(table_name, ex).await,
        SensorDataStorageType::RingBufferCount => RING_BUFFER_COUNT_OBJ.on_remove(table_name, ex).await,
        SensorDataStorageType::RingBufferInterval => RING_BUFFER_INTERVAL_OBJ.on_remove(table_name, ex).await,
    }
}

// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SensorDataStorageType {
    Default = 0, // Unlimited storage of tuples
    RingBufferCount = 1, // Only stores the last x amount of tuples
    RingBufferInterval = 2, // Only stores tuples of the last x minutes
}

trait SensorDataStorageStrategy {
    /// Called when this strategy is registered for a sensor.
    async fn on_initialize(&self, table_name: String, params: Option<Map<String, Value>>, ex: &mut PgConnection) -> Result<()>;

    /// Called when this strategy is unregistered from the sensor.
    async fn on_remove(&self, table_name: String, ex: &mut PgConnection) -> Result<()>;
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SensorDataStorageCfg {
    pub variant: SensorDataStorageType,
    pub params: Option<Map<String, Value>>
}

// -------------------------------------------- Default --------------------------------------------

struct DefaultSDS;

impl SensorDataStorageStrategy for DefaultSDS {
    async fn on_initialize(&self, _: String, params: Option<Map<String, Value>>, _: &mut PgConnection) -> Result<()> {
        if params.is_some() && params.unwrap().len() != 0 {
            anyhow::bail!("Wrong params! Should be None or no params!");
        }

        Ok(())
    }

    async fn on_remove(&self, _: String, _: &mut PgConnection) -> Result<()> {
        Ok(())
    }
}

// --------------------------------------- Ring Buffer Count ---------------------------------------

struct RingBufferCountSDS;

/// Creates a DB trigger for managing the ring buffer count.
/// Expected params: {"count": 10}
impl SensorDataStorageStrategy for RingBufferCountSDS {
    async fn on_initialize(&self, table_name: String, params: Option<Map<String, Value>>, ex: &mut PgConnection) -> Result<()> {
        if params.is_none() {
            anyhow::bail!("Invalid params! Expected count value!");
        }
        
        let params = params.unwrap();
        
        let count = params.get("count");
        
        if count.is_none() || params.len() > 1 {
            anyhow::bail!("Invalid params! Expected only count value!");
        }

        let count = count.unwrap();

        if !count.is_i64() || count.as_i64().unwrap() <= 0 {
            anyhow::bail!("Count is not an integer > 0!");
        }

        let res = sqlx::query(format!("select create_ring_buffer_count('{}', {});", table_name, count).as_str())
            .execute(&mut *ex) .await
            .map_err(|err: sqlx::Error| err.to_string());

        if let Err(err) = res {
            println!("Failed to initialize RingBufferCountSDS!");
            anyhow::bail!(err)
        }
        
        Ok(())
    }

    async fn on_remove(&self, table_name: String, ex: &mut PgConnection) -> Result<()> {
        let res = sqlx::query(format!("drop function check_data_storage_{}() cascade;", table_name).as_str())
            .execute(&mut *ex) .await
            .map_err(|err: sqlx::Error| err.to_string());

        if let Err(err) = res {
            println!("Failed to remove RingBufferCountSDS!");
            anyhow::bail!(err)
        }

        Ok(())
    }
}

// -------------------------------------- Ring Buffer Interval -------------------------------------

struct RingBufferIntervalSDS;

/// Creates a DB trigger for managing the ring buffer interval.
/// Expected params: {"interval": 42.42} in minutes
impl SensorDataStorageStrategy for RingBufferIntervalSDS {
    async fn on_initialize(&self, table_name: String, params: Option<Map<String, Value>>, ex: &mut PgConnection) -> Result<()> {
        if params.is_none() {
            anyhow::bail!("Invalid params! Expected interval value!");
        }

        let params = params.unwrap();
        
        let range = params.get("interval");

        if range.is_none() || params.len() > 1 {
            anyhow::bail!("Invalid params! Expected only interval value!");
        }
        
        let range = range.unwrap();
        
        if !range.is_i64() && !range.is_f64() {
            anyhow::bail!("Interval is not a number!");
        }

        let res = sqlx::query(format!("select create_ring_buffer_interval('{}', {});", table_name, range).as_str())
            .execute(&mut *ex) .await
            .map_err(|err: sqlx::Error| err.to_string());

        if let Err(err) = res {
            println!("Failed to initialize RingBufferIntervalSDS!");
            anyhow::bail!(err)
        }

        Ok(())
    }

    async fn on_remove(&self, table_name: String, ex: &mut PgConnection) -> Result<()> {
        let res = sqlx::query(format!("drop function check_data_storage_{}() cascade;", table_name).as_str())
            .execute(&mut *ex) .await
            .map_err(|err: sqlx::Error| err.to_string());

        if let Err(err) = res {
            println!("Failed to remove RingBufferIntervalSDS!");
            anyhow::bail!(err)
        }

        Ok(())
    }
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use crate::state::AppState;
    use async_std::task;
    use serde_json::{json};
    use sqlx::PgPool;
    use uuid::Uuid;
    use crate::database::sensor_db;
    use crate::features::cache;
    use crate::features::sensor_data_storage::{SensorDataStorageCfg, SensorDataStorageType};
    use crate::handler::models::requests::{EditSensorRequest, SensorDataRequest};
    use crate::test_utils::tests::{create_test_app, create_test_sensors};

    fn edit_sensor_req(storage: SensorDataStorageCfg) -> EditSensorRequest {
        EditSensorRequest {
            name: "NewName".to_string(),
            description: Some("My new sensor!".to_string()),
            position: Some((50.0, 10.0)),
            permissions: vec![],
            storage
        }
    }

    async fn add_dummy_data(amount: u32, index_off: u32, sleep_ms: u64, sensor_id: Uuid, state: &AppState) {
        for i in 0..amount {
            let payload = {
                json!({
                    "col1": i + index_off,
                    "col2": 56.789,
                    "col3": "Hello",
                })
            };

            sensor_db::add_sensor_data(sensor_id, &payload, &state).await.unwrap();

            task::sleep(core::time::Duration::from_millis(sleep_ms)).await; // A little bit of time for time tracking
        }
    }

    #[sqlx::test(migrations = "../migrations", fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql"))]
    async fn test_default(pool: PgPool) {
        let (_, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let sensor_id = test_sens.iter().find(|(name, _)| name == "MySensor5").unwrap().1;

        // Perform data storage change

        // Invalid param
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::Default, params: json!({"ff": 10}).as_object().cloned() }), &state).await.is_err());

        // Valid params
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::Default, params: json!({}).as_object().cloned() }), &state).await.is_ok());
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::Default, params: None }), &state).await.is_ok());

        // Valid change

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::Default,
            params: None
        }), &state).await.unwrap();
        
        // Validate retrieval from db
        let sensor = cache::request_sensor(sensor_id, &state).await.unwrap();
        assert_eq!(sensor.storage_type, SensorDataStorageType::Default);
        assert_eq!(sensor.storage_params, None);

        add_dummy_data(15, 0, 0, sensor_id, &state).await;

        // Check if all data remains

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();
        let elms = data.as_array().unwrap();
        assert_eq!(elms.len(), 15);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 0 + index as i64);
        }
    }

    #[sqlx::test(migrations = "../migrations", fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql"))]
    async fn test_ring_buffer_count(pool: PgPool) {
        let (_, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let sensor_id = test_sens.iter().find(|(name, _)| name == "MySensor5").unwrap().1;

        add_dummy_data(10, 0, 0, sensor_id, &state).await; // Initial dummy data

        // Perform data storage change

        // Invalid param
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferCount, params: json!({"asd": 10}).as_object().cloned() }), &state).await.is_err());

        // Wrong param val
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferCount, params: json!({"count": "123"}).as_object().cloned() }), &state).await.is_err());
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferCount, params: json!({"count": 1234.1234}).as_object().cloned() }), &state).await.is_err());

        // To many params
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferCount, params: json!({"count": 10, "prop": "value"}).as_object().cloned() }), &state).await.is_err());

        // Valid change

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::RingBufferCount,
            params: json!({"count": 10}).as_object().cloned()
        }), &state).await.unwrap();

        // Validate retrieval from db
        let sensor = cache::request_sensor(sensor_id, &state).await.unwrap();
        assert_eq!(sensor.storage_type, SensorDataStorageType::RingBufferCount);
        assert_eq!(sensor.storage_params, json!({"count": 10}).as_object().cloned());

        add_dummy_data(15, 0, 0, sensor_id, &state).await;

        // Check if only the last 10 tuples are present (according to strategy)

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();

        let elms = data.as_array().unwrap();

        assert_eq!(elms.len(), 10);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 5 + index as i64);
        }

        // Increase limit to 15 tuples and add new tuples

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::RingBufferCount,
            params: json!({"count": 15}).as_object().cloned()
        }), &state).await.unwrap();

        add_dummy_data(15, 15, 0, sensor_id, &state).await;

        // Check if only the last 15 tuples are present (according to strategy)

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();

        let elms = data.as_array().unwrap();

        assert_eq!(elms.len(), 15);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 15 + index as i64);
        }
    }

    #[sqlx::test(migrations = "../migrations", fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql"))]
    async fn test_ring_buffer_range(pool: PgPool) {
        let (_, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let sensor_id = test_sens.iter().find(|(name, _)| name == "MySensor5").unwrap().1;

        add_dummy_data(10, 0, 0, sensor_id, &state).await; // Initial dummy data

        // Perform data storage change

        // Invalid param
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferInterval, params: json!({"asdc": 10}).as_object().cloned() }), &state).await.is_err());

        // Wrong param val
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferInterval, params: json!({"interval": "123"}).as_object().cloned() }), &state).await.is_err());

        // Valid param vals
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferInterval, params: json!({"interval": 123}).as_object().cloned() }), &state).await.is_ok());
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferInterval, params: json!({"interval": 123.123}).as_object().cloned() }), &state).await.is_ok());

        // To many params
        assert!(sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg { variant: SensorDataStorageType::RingBufferInterval, params: json!({"interval": 10, "prop": "value"}).as_object().cloned() }), &state).await.is_err());

        // Valid change

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::RingBufferInterval,
            params: json!({"interval": 600.0 / 60_000.0}).as_object().cloned()
        }), &state).await.unwrap();

        // Validate retrieval from db
        let sensor = cache::request_sensor(sensor_id, &state).await.unwrap();
        assert_eq!(sensor.storage_type, SensorDataStorageType::RingBufferInterval);
        assert_eq!(sensor.storage_params, json!({"interval": 600.0 / 60_000.0}).as_object().cloned());

        add_dummy_data(6, 0, 10, sensor_id, &state).await; // Tuples that will be evicted
        task::sleep(core::time::Duration::from_millis(600)).await; // Clear delay to avoid fluctuation in runtimes
        add_dummy_data(6, 6, 10, sensor_id, &state).await; // Tuples we will keep (interval of 600ms)

        // Check if only the last 6 tuples are present

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();

        let elms = data.as_array().unwrap();

        assert_eq!(elms.len(), 6);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 6 + index as i64);
        }

        // Adapt to accep twice as many tuples (12)

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::RingBufferInterval,
            params: json!({"interval":  1_200.0 / 60_000.0
            }).as_object().cloned() }), &state).await.unwrap();

        // Add more tuples

        task::sleep(core::time::Duration::from_millis(1_200)).await; // Clear delay to avoid fluctuation in runtimes
        add_dummy_data(12, 12, 10, sensor_id, &state).await; // Tuples we will keep (interval of 1200ms)

        // Check if only the last 12 tuples are present

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();

        let elms = data.as_array().unwrap();

        assert_eq!(elms.len(), 12);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 12 + index as i64);
        }
    }

    #[sqlx::test(migrations = "../migrations", fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql"))]
    async fn test_strategy_switch(pool: PgPool) {
        let (_, state) = create_test_app(pool).await;

        let test_sens = create_test_sensors(&state).await;
        let sensor_id = test_sens.iter().find(|(name, _)| name == "MySensor5").unwrap().1;

        add_dummy_data(10, 0, 0, sensor_id, &state).await; // Initial dummy data

        // --- Switch from Default to Count ---

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::RingBufferCount,
            params: json!({"count": 10}).as_object().cloned()
        }), &state).await.unwrap();

        add_dummy_data(15, 0, 0, sensor_id, &state).await;

        // Check if only the last 10 tuples are present (according to strategy)

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();
        let elms = data.as_array().unwrap();
        assert_eq!(elms.len(), 10);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 5 + index as i64);
        }

        // --- Switch from Count to Interval ---

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::RingBufferInterval,
            params: json!({"interval": 600.0 / 60_000.0}).as_object().cloned()
        }), &state).await.unwrap();

        // Strategy switch should not affect previous tuples immediately
        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();
        let elms = data.as_array().unwrap();
        assert_eq!(elms.len(), 10);

        // Add new data, evicting old
        add_dummy_data(6, 15, 10, sensor_id, &state).await; // Tuples that will be evicted
        task::sleep(core::time::Duration::from_millis(600)).await; // Clear delay to avoid fluctuation in runtimes
        add_dummy_data(6, 21, 10, sensor_id, &state).await; // Tuples we will keep (interval of 600ms)

        // Check if only the last 6 tuples are present

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();
        let elms = data.as_array().unwrap();
        assert_eq!(elms.len(), 6);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 21 + index as i64);
        }

        // --- Switch from Interval to Default ---

        sensor_db::edit_sensor(sensor_id, edit_sensor_req(SensorDataStorageCfg {
            variant: SensorDataStorageType::Default,
            params: json!({}).as_object().cloned()
        }), &state).await.unwrap();

        // Check if previous tuples remain + new ones

        add_dummy_data(10, 27, 0, sensor_id, &state).await;

        let data = sensor_db::get_data(sensor_id, SensorDataRequest { limit: None, ordering: None, from: None, to: None}, &state).await.unwrap();
        let elms = data.as_array().unwrap();
        assert_eq!(elms.len(), 16);

        for (index, elm) in elms.iter().enumerate() {
            assert_eq!(elm.as_object().unwrap().get("col1").unwrap().as_i64().unwrap(), 21 + index as i64);
        }
    }
}