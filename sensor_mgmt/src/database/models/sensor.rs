use crate::utils::uuid_schema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;
use crate::database::models::sensor_perm::SensorPermission;
use crate::features::sensor_data_storage::SensorDataStorageType;

/// Structure for JSON data returned from getting sensor information.
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct FullSensorInfo {
    #[schema(schema_with = uuid_schema)]
    pub id: uuid::Uuid,
    pub name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub tbl_name: String,
    pub position: Option<(f64,f64)>,
    pub description: Option<String>,
    #[schema(schema_with = uuid_schema)]
    pub owner: Option<uuid::Uuid>,
    pub columns: Vec<SensorColumn>,
    pub permissions: Vec<SensorPermission>,
    pub storage_type: SensorDataStorageType,
    pub storage_params: Option<Map<String, Value>>
}

impl FullSensorInfo {
    pub fn is_owner(&self, user_id: uuid::Uuid) -> bool {
        self.owner.is_some() && self.owner.unwrap() == user_id
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ShortSensorInfo {
    #[schema(schema_with = uuid_schema)]
    pub id: uuid::Uuid,
    pub name: String,
}

/// The possible types of values stored in the sensor data table.
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Copy, Clone, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
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
            _ => panic!("Unknown value: {}", v)
        }
    }
}

/// Information about a column in the sensor data table.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SensorColumn {
	pub name: String,         // column name
    pub val_type: ColumnType, // column type
    pub val_unit: String,     // measurement unit
}
