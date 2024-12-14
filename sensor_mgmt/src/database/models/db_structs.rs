use serde_derive::{Deserialize, Serialize};
use utoipa::ToSchema;

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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum DBOrdering {
    DEFAULT = 0,
    ASC = 1,
    DESC = 2,
}
