use derive_more::derive::Display;
use serde_derive::{Deserialize, Serialize};
use sqlx::{Decode, Type};
use sqlx::postgres::{PgTypeInfo, PgValueRef};
use utoipa::ToSchema;
use crate::database::models::sensor::ColumnType;

/// Modes for database operations - used for access control to sensor measurements.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DBOperation {
    INFO = 0,  // obtain information about sensor
    READ = 1,  // read measurement data
    WRITE = 2, // write measurement data
}

impl From<String> for DBOperation {
    fn from(s: String) -> Self {
        match s.as_str() {
            "INFO" => DBOperation::INFO,
            "READ" => DBOperation::READ,
            "WRITE" => DBOperation::WRITE,
            _ => panic!("Invalid value for DBOperation: {}", s),
        }
    }
}

impl<'r> Decode<'r, sqlx::Postgres> for DBOperation {
    fn decode(value: PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s: &str = <&str as Decode<sqlx::Postgres>>::decode(value)?;
        Ok(DBOperation::from(s.to_string()))
    }
}

impl Type<sqlx::Postgres> for DBOperation {
    fn type_info() -> PgTypeInfo {
        <&str as Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        // Allow anything compatible with &str (TEXT, VARCHAR, etc.)
        <&str as Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl DBOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            DBOperation::INFO => "INFO",
            DBOperation::READ => "READ",
            DBOperation::WRITE => "WRITE",
        }
    }

    pub fn all() -> Vec<DBOperation> {
        vec![DBOperation::INFO, DBOperation::READ, DBOperation::WRITE]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Display)]
#[serde(rename_all = "UPPERCASE")]
pub enum DBOrdering {
    DEFAULT = 0,
    ASC = 1,
    DESC = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Display)]
#[serde(rename_all = "UPPERCASE")]
pub enum DBAggregation {
    MIN = 0,
    MAX = 1,
    SUM = 2,
    AVG = 3,
    COUNT = 4,
}

impl From<String> for DBAggregation {
    fn from(s: String) -> Self {
        match s.as_str() {
            "MIN" => DBAggregation::MIN,
            "MAX" => DBAggregation::MAX,
            "SUM" => DBAggregation::SUM,
            "AVG" => DBAggregation::AVG,
            "COUNT" => DBAggregation::COUNT,
            
            _ => panic!("Invalid value for DBAggregation: {}", s),
        }
    }
}

impl DBAggregation {
    pub fn as_str(&self) -> &'static str {
        match self {
            DBAggregation::MIN => "MIN",
            DBAggregation::MAX => "MAX",
            DBAggregation::SUM => "SUM",
            DBAggregation::AVG => "AVG",
            DBAggregation::COUNT => "COUNT",
        }
    }
    
    pub fn get_result_type(&self, in_type: ColumnType) -> ColumnType {
        match self {
            DBAggregation::MIN => in_type,
            DBAggregation::MAX => in_type,
            DBAggregation::SUM => {
                // Only valid for numeric types
                if in_type == ColumnType::INT || in_type == ColumnType::FLOAT {
                    return in_type;
                }
                
                ColumnType::UNKNOWN
            },
            DBAggregation::AVG => {
                // Only valid for numeric types
                if in_type == ColumnType::INT || in_type == ColumnType::FLOAT {
                    return ColumnType::FLOAT;
                }

                ColumnType::UNKNOWN
            },
            DBAggregation::COUNT => ColumnType::INT,
        }
    }
    
    pub fn as_db_op(&self) -> &str {
        self.as_str()
    }
}