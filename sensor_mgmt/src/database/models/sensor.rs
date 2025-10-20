use crate::database::models::sensor_perm::SensorPermission;
use crate::features::sensor_data_storage::SensorDataStorageType;
use crate::utils::uuid_schema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

/// Structure for JSON data returned from getting sensor information.
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct FullSensorInfo {
    #[schema(schema_with = uuid_schema)]
    pub id: uuid::Uuid,
    pub name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub tbl_name: String,
    pub position: Option<(f64, f64)>,
    pub description: Option<String>,
    #[schema(schema_with = uuid_schema)]
    pub owner: Option<uuid::Uuid>,
    pub columns: Vec<SensorColumn>,
    pub permissions: Vec<SensorPermission>,
    pub storage_type: SensorDataStorageType,
    pub storage_params: Option<Map<String, Value>>,
}

impl FullSensorInfo {
    pub fn is_owner(&self, user_id: uuid::Uuid) -> bool {
        self.owner.is_some() && self.owner.unwrap() == user_id
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema, sqlx::FromRow)]
pub struct ShortSensorInfo {
    #[schema(schema_with = uuid_schema)]
    pub id: uuid::Uuid,
    pub name: String,
    // Sensor Position
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// The possible types of values stored in the sensor data table.
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Copy, Clone, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[schema(
    description = "NOTE: Any INT column will be bound to rust i32. Any FLOAT column will be bound to rust f64. STRING is VARCHAR(50)"
)]
pub enum ColumnType {
    UNKNOWN = 0,
    INT = 1,
    FLOAT = 2,
    STRING = 3,
}

/// A helper function for converting int values from the database to ColumnType
impl ColumnType {
    pub fn from_integer(v: i32) -> Self {
        match v {
            0 => Self::UNKNOWN,
            1 => Self::INT,
            2 => Self::FLOAT,
            3 => Self::STRING,
            _ => panic!("Unknown value: {}", v),
        }
    }

    pub fn to_sql_value(&self) -> i32 {
        *self as i32
    }

    pub fn to_sql_type(&self) -> &str {
        match &self {
            ColumnType::INT => "INTEGER",
            ColumnType::FLOAT => "FLOAT",
            _ => "VARCHAR(255)",
        }
    }
}

/// The possible types of ingest data modes for a sensor data column.
/// Literal: Stores the ingested column value directly into the db
/// Incremental: Stores the sum of the last column value and the ingested value into the db
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Copy, Clone, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum ColumnIngest {
    /// Store the provided value directly into the column
    LITERAL = 0,
    /// Increment the last value of the column by the provided new value
    INCREMENTAL = 1,
}

impl ColumnIngest {
    pub fn from_integer(v: i32) -> Self {
        match v {
            0 => Self::LITERAL,
            1 => Self::INCREMENTAL,
            _ => panic!("Unknown value: {}", v),
        }
    }

    pub fn to_sql_value(&self) -> i32 {
        *self as i32
    }
}

/// Information about a column in the sensor data table.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SensorColumn {
    pub name: String,             // column name
    pub val_type: ColumnType,     // column type
    pub val_unit: String,         // measurement unit
    pub val_ingest: ColumnIngest, // data ingest mode
}
