use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::ToSchema;

#[repr(u32)]
pub enum UserSensorPerm {
    Info = 1 << 0,
    Read = 1 << 1,
    Write = 1 << 2,
    Edit = 1 << 3,
    Delete = 1 << 4,
    ApiKeyRead = 1 << 5,
    ApiKeyWrite = 1 << 6,
}

/// Bit map with [0] Info, [1] Read, [2] Write, [3] Edit, [4] Delete, [5] ApiKeyRead, [6] ApiKeyWrite
#[derive(Serialize, Debug, Clone, Copy, Deserialize, ToSchema)]
pub struct UserSensorPermissions {
    #[serde(serialize_with = "serialize_permissions", deserialize_with = "deserialize_permissions")]
    #[schema(example = "all bits active == 127")]
    bit_set: u32,
}

// Custom serializer for the `permissions` field
fn serialize_permissions<S>(bit_set: &u32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let readable = UserSensorPermissions::to_human_readable(*bit_set);
    serializer.serialize_str(&readable)
}
// Custom deserializer for the `permissions` field
fn deserialize_permissions<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    UserSensorPermissions::from_human_readable(&s).ok_or_else(|| serde::de::Error::custom("invalid permissions string"))
}

impl UserSensorPermissions {
    pub fn new() -> Self {
        UserSensorPermissions { bit_set: 0 }
    }

    // Add a permission
    pub fn add(&mut self, permission: UserSensorPerm) {
        self.bit_set |= permission as u32;
    }
    
    // Set full permissions
    pub fn add_all(&mut self) {
        self.add(UserSensorPerm::Info);
        self.add(UserSensorPerm::Read);
        self.add(UserSensorPerm::Write);
        self.add(UserSensorPerm::Edit);
        self.add(UserSensorPerm::Delete);
        self.add(UserSensorPerm::ApiKeyRead);
        self.add(UserSensorPerm::ApiKeyWrite);
    }

    // Remove a permission
    pub fn remove(&mut self, permission: UserSensorPerm) {
        self.bit_set &= !(permission as u32);
    }

    // Check if a permission is set
    pub fn has(&self, permission: UserSensorPerm) -> bool {
        self.bit_set & (permission as u32) != 0
    }

    pub fn has_all(&self) -> bool {
        self.has(UserSensorPerm::Info) &&
            self.has(UserSensorPerm::Read) &&
            self.has(UserSensorPerm::Write) &&
            self.has(UserSensorPerm::Edit) &&
            self.has(UserSensorPerm::Delete) &&
            self.has(UserSensorPerm::ApiKeyRead) &&
            self.has(UserSensorPerm::ApiKeyWrite)
    }

    pub fn to_human_readable(bitmask: u32) -> String {
        let mut permissions = Vec::new();

        if bitmask & (1 << 0) != 0 {
            permissions.push("Info");
        }
        if bitmask & (1 << 1) != 0 {
            permissions.push("Read");
        }
        if bitmask & (1 << 2) != 0 {
            permissions.push("Write");
        }
        if bitmask & (1 << 3) != 0 {
            permissions.push("Edit");
        }
        if bitmask & (1 << 4) != 0 {
            permissions.push("Delete");
        }
        if bitmask & (1 << 5) != 0 {
            permissions.push("ApiKeyRead");
        }
        if bitmask & (1 << 6) != 0 {
            permissions.push("ApiKeyWrite");
        }

        // Join the permission names with commas
        permissions.join(", ")
    }

    pub fn from_human_readable(permissions_str: &str) -> Option<u32> {
        let mut bitmask = 0;

        for permission in permissions_str.split(',') {
            let trimmed = permission.trim();
            match trimmed {
                "Info" => bitmask |= 1 << 0,
                "Read" => bitmask |= 1 << 1,
                "Write" => bitmask |= 1 << 2,
                "Edit" => bitmask |= 1 << 3,
                "Delete" => bitmask |= 1 << 4,
                "ApiKeyRead" => bitmask |= 1 << 5,
                "ApiKeyWrite" => bitmask |= 1 << 6,
                _ => return None, // Invalid permission
            }
        }

        Some(bitmask)
    }
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use sqlx::PgPool;
    use crate::features::user_sens_perm::*;

    #[sqlx::test]
    async fn test_permission_set(_: PgPool) {
        let mut permissions = UserSensorPermissions::new();

        // Add permissions
        
        permissions.add(UserSensorPerm::Read);
        permissions.add(UserSensorPerm::Write);
        
        assert!(permissions.has(UserSensorPerm::Read));
        assert!(permissions.has(UserSensorPerm::Write));
        assert!(!permissions.has(UserSensorPerm::Delete));
        
        // Remove permissions

        permissions.remove(UserSensorPerm::Read);

        assert!(!permissions.has(UserSensorPerm::Read));
        
        // Check all permissions

        let mut permissions = UserSensorPermissions::new();
        
        permissions.add_all();
        
        assert!(permissions.has_all());
    }
}