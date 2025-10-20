use crate::database::models::data_chain::DataChain;
use crate::database::models::db_structs::{DBAggregation, DBOperation, DBOrdering};
use crate::database::models::events::EventHandler;
use crate::database::models::sensor::SensorColumn;
use crate::features::config::TIMESTAMP_FORMAT;
use crate::features::sensor_data_storage::SensorDataStorageCfg;
use crate::utils::uuid_schema;
use crate::utils::{query_param_vec_deserializer, serialize_vec_query_params, QueryParam};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

/// Query Parameters
///
/// The timestamp uses %Y-%m-%dT%H:%M:%S%.3f as the format in the actual db query
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, Default)]
pub struct DataLoadRequestParams {
    /// The uuid of the key that should be used to verify the authorization to load the data
    /// Leave empty if the data is publicly available
    #[schema(schema_with = uuid_schema)]
    pub key: Option<uuid::Uuid>,
    /// How many rows to return
    pub limit: Option<i32>,

    /// How to order the result values, by default no ordering is applied
    pub ordering: Option<DBOrdering>,
    /// Which data column to use for ordering the results, by default time column "created_at" is used
    pub order_col: Option<String>,

    // NaiveDateTime is parsed with a trailing "Z" for the TimeZone which cant be parsed by
    // serde_json since NaiveDateTime does not contain a timezone!
    /// ISO 8601 timestamp
    #[schema(example = "2025-02-11T08:27:17")]
    pub from: Option<chrono::NaiveDateTime>,
    /// ISO 8601 timestamp
    #[schema(example = "2025-02-11T08:27:17")]
    pub to: Option<chrono::NaiveDateTime>,

    /// Specifies, how to 'from' interval border should be considered. >= or >
    pub from_inclusive: Option<bool>,

    /// Specifies, how to 'to' interval border should be considered. <= or <
    pub to_inclusive: Option<bool>,

    /// Define, which data columns to retrieve with which preprocessing. Serialized to: cols=col1.min,col2.avg
    /// By default, all data columns are returned (including time column created_at).
    /// If aggregation is defined, the time_grouping value must be specified! [All data cols must provide an aggregation!]
    #[serde(default, deserialize_with = "query_param_vec_deserializer")]
    pub cols: Option<Vec<DataLoadRequestColumns>>,
    /// Define the time interval [s] used for grouping the result values. E.g. 3.600=1hour
    pub time_grouping: Option<u32>,
}

impl DataLoadRequestParams {
    /// Method to convert struct fields into a vector when used for query params
    pub fn to_vector(&self) -> Vec<(String, String)> {
        let mut vec = Vec::new();

        if let Some(v) = &self.key {
            vec.push(("key".to_string(), v.to_string()));
        }

        if let Some(v) = &self.limit {
            vec.push(("limit".to_string(), v.to_string()));
        }

        if let Some(v) = &self.ordering {
            vec.push(("ordering".to_string(), v.to_string()));
        }

        if let Some(v) = &self.order_col {
            vec.push(("order_col".to_string(), v.to_string()));
        }

        if let Some(v) = &self.from {
            vec.push(("from".to_string(), v.format(TIMESTAMP_FORMAT).to_string()));
        }

        if let Some(v) = &self.to {
            vec.push(("to".to_string(), v.format(TIMESTAMP_FORMAT).to_string()));
        }

        if let Some(v) = &self.from_inclusive {
            vec.push(("from_inclusive".to_string(), v.to_string()));
        }

        if let Some(v) = &self.to_inclusive {
            vec.push(("to_inclusive".to_string(), v.to_string()));
        }

        if let Some(v) = &self.cols {
            vec.push(("cols".to_string(), serialize_vec_query_params(v)));
        }

        if let Some(v) = &self.time_grouping {
            vec.push(("time_grouping".to_string(), v.to_string()));
        }

        vec
    }
}

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, Default)]
pub struct DataLoadRequestColumns {
    pub name: String,
    pub aggregation: Option<DBAggregation>,
}

impl QueryParam for DataLoadRequestColumns {
    /// Converts from col_name.aggregation into DataLoadRequestColumn object
    fn from_query_param(param: String) -> Self {
        let mut parts = param.splitn(2, '.');
        let name = parts.next().unwrap_or("").to_string();
        let aggregation = parts
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| DBAggregation::from(s.to_string()));

        DataLoadRequestColumns { name, aggregation }
    }

    /// Converts to col_name.aggregation
    fn to_query_param(&self) -> String {
        match &self.aggregation {
            Some(agg) => format!("{}.{}", self.name, agg),
            None => format!("{}", self.name),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, Default)]
pub struct DataIngestRequestParams {
    #[schema(schema_with = uuid_schema)]
    pub key: Option<uuid::Uuid>,
}

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone)]
pub struct CreateSensorRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<(f64, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub permissions: Vec<SensorPermissionRequest>,
    pub columns: Vec<SensorColumn>,
    pub storage: SensorDataStorageCfg,
    // TODO: Later we could specify an ingest method (http, mqtt, ...)
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct EditSensorRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<(f64, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub permissions: Vec<SensorPermissionRequest>,
    pub storage: SensorDataStorageCfg,
    // TODO: Later we could update the ingest method (http, mqtt, ...)
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SensorPermissionRequest {
    #[schema(schema_with = uuid_schema)]
    pub role_id: uuid::Uuid,
    pub operations: Vec<DBOperation>,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct SensorDataIngestEntry {
    /// ISO 8601 timestamp
    #[schema(example = "2025-02-11T08:27:17")]
    pub timestamp: Option<chrono::NaiveDateTime>,

    #[serde(flatten)]
    pub data: HashMap<String, Value>,
}

impl SensorDataIngestEntry {
    pub fn from_json(col_data: Value, timestamp: Option<chrono::NaiveDateTime>) -> Self {
        let key_vals = col_data.as_object().unwrap();

        let mut col_data = HashMap::new();

        for (key, val) in key_vals.iter() {
            col_data.insert(key.to_owned(), val.to_owned());
        }

        SensorDataIngestEntry {
            timestamp,
            data: col_data,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, ToSchema, Default)]
pub struct SensorDataDeletionParams {
    #[schema(schema_with = uuid_schema)]
    pub key: Option<uuid::Uuid>,

    /// ISO 8601 timestamp
    #[schema(example = "2025-02-11T08:27:17")]
    pub from: Option<chrono::NaiveDateTime>,

    /// ISO 8601 timestamp
    #[schema(example = "2025-02-11T08:27:17")]
    pub to: Option<chrono::NaiveDateTime>,

    /// Specifies, how to 'from' interval border should be considered. >= or >
    pub from_inclusive: Option<bool>,

    /// Specifies, how to 'to' interval border should be considered. <= or <
    pub to_inclusive: Option<bool>,

    pub purge: Option<bool>,
}

impl SensorDataDeletionParams {
    /// Method to convert struct fields into a vector when used for query params
    pub fn to_vector(&self) -> Vec<(String, String)> {
        let mut vec = Vec::new();

        if let Some(v) = &self.key {
            vec.push(("key".to_string(), v.to_string()));
        }

        if let Some(v) = &self.from {
            vec.push(("from".to_string(), v.format(TIMESTAMP_FORMAT).to_string()));
        }

        if let Some(v) = &self.to {
            vec.push(("to".to_string(), v.format(TIMESTAMP_FORMAT).to_string()));
        }

        if let Some(v) = &self.from_inclusive {
            vec.push(("from_inclusive".to_string(), v.to_string()));
        }

        if let Some(v) = &self.to_inclusive {
            vec.push(("to_inclusive".to_string(), v.to_string()));
        }

        if let Some(v) = &self.purge {
            vec.push(("purge".to_string(), v.to_string()));
        }

        vec
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterUserRequest {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub operation: DBOperation,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct EditUserInfoRequest {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct CreateRoleRequest {
    pub name: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct CreateDataTransformScriptRequest {
    pub name: String,
    pub script: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct UpdateDataTransformScriptRequest {
    pub name: String,
    pub script: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct CreateEventHandlerRequest {
    pub name: String,
    pub filter: String,
    pub url: String,
    pub method: String,
}

impl CreateEventHandlerRequest {
    pub fn to_new_handler(&self) -> EventHandler {
        EventHandler {
            id: Uuid::new_v4(),
            name: self.name.clone(),
            filter: self.filter.clone(),
            url: self.url.clone(),
            method: self.method.clone(),
        }
    }
}

// Available Protos for data ingest
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TransportProto {
    HTTP,
    MQTT,
}
impl TransportProto {
    pub fn iterator() -> std::slice::Iter<'static, TransportProto> {
        static PROTOS: [TransportProto; 2] = [TransportProto::HTTP, TransportProto::MQTT];
        PROTOS.iter()
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct UpSertDataTransformScriptRequest {
    pub name: String,
    pub script: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, ToSchema)]
pub struct SetDataChainRequest {
    pub chain: DataChain,
}
