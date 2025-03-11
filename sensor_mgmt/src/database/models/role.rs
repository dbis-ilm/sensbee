use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::{uuid, Uuid};

use crate::utils::uuid_schema;

pub const ROLE_SYSTEM_ADMIN: Uuid = uuid!("0e804d35-c8e3-49ee-86d4-3e556a82a1af");
pub const ROLE_SYSTEM_USER: Uuid = uuid!("72122092-1154-4189-8dde-d72b663b55eb");
pub const ROLE_SYSTEM_GUEST: Uuid = uuid!("51fd9bb7-3214-4089-adb9-474eb82b447a");

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Clone, ToSchema)]
pub struct Role {
    #[schema(schema_with = uuid_schema)]
    pub id: uuid::Uuid,
    pub name: String,
    pub system: bool,
}

impl Role {
    pub fn is_admin(&self) -> bool {
        self.id.eq(&ROLE_SYSTEM_ADMIN)
    }
}
