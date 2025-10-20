use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::utils::uuid_schema;

/// A Data chain is specific to a sensor.
/// One of the in/outbound options should be not None!
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct DataChain {
    // The inbound part of the chain has an optional data transformer
    #[schema(schema_with = uuid_schema)]
    pub inbound: Option<Uuid>,
    // The outbound part may have any number of outbound chains
    pub outbound: Option<Vec<DataChainOutbound>>,
}

/// A single outbound chain ends in an event handler. It may have an optional data transformer that transforms the data before it is send to the event handler.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, FromRow)]
pub struct DataChainOutbound {
    #[schema(schema_with = uuid_schema)]
    pub event_handler_id: Uuid,
    #[schema(schema_with = uuid_schema)]
    pub data_transformer_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DataChainInternal {
    pub sensor_id: Uuid,
    pub event_handler_id: Uuid,
    pub data_transformer_id: Option<Uuid>,
}
