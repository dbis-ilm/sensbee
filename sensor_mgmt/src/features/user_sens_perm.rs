use serde::{Serialize, Deserialize};
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


#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct UserSensorPermissions {
    bit_set: u32,
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