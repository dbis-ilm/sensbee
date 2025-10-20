use crate::database::models::api_key::ApiKey;
use crate::database::models::role::Role;
use crate::database::models::sensor::FullSensorInfo;
use crate::database::models::user::UserInfo;
use crate::database::{role_db, sensor_db, user_db};
use crate::state::AppState;
use derive_more::derive::Display;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

#[cfg(feature = "cache_sync")]
use super::cache_sync;

// Thread-safe data cache across the application
// It must be ensured, that the locks are not held across await points, since this is not a
// Send+Sync safe implementation for processing across different threads/tasks.

/* ------------------------------------------------------------------------------------------------- */

// TODO: We should add a test for multi-threaded accesses

#[derive(Display, Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub enum MapType {
    Roles,
    Users,
    Sensors,
    ApiKeys,
}

pub fn purge_all(state: &AppState) {
    purge_all_users(state);
    purge_all_sensors(state);
    purge_all_keys(state);
    purge_all_roles(state);
}

pub struct CachedData {
    pub roles: RwLock<HashMap<Uuid, Role>>,
    pub users: RwLock<HashMap<Uuid, UserInfo>>,
    pub sensors: RwLock<HashMap<Uuid, FullSensorInfo>>,
    pub api_keys: RwLock<HashMap<Uuid, ApiKey>>,
}

pub fn new_cache() -> CachedData {
    CachedData {
        roles: RwLock::default(),
        users: RwLock::default(),
        sensors: RwLock::default(),
        api_keys: RwLock::default(),
    }
}

/* --------------------------------------------- Roles ---------------------------------------------------- */

pub async fn request_role(role_id: uuid::Uuid, state: &AppState) -> Option<Role> {
    {
        let roles = state.cache.roles.read().unwrap();

        // Check if role is present in cache

        if let Some(v) = roles.get(&role_id) {
            return Some(v.clone());
        }
    }

    // Otherwise, fetch roles from DB and insert them into the cache

    let r = role_db::get_role(role_id, &state).await;

    if r.is_err() {
        return None;
    }

    {
        let role = r.unwrap();

        let mut roles = state.cache.roles.write().unwrap();

        roles.insert(role_id, role.clone());

        Some(role)
    }
}

pub fn purge_role(role_id: uuid::Uuid, state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.roles.write().unwrap().remove(&role_id);

    // If synced caching is enabled we need to instead purge the value from all caches
    #[cfg(feature = "cache_sync")]
    state.sync.send(MapType::Roles, role_id.to_string());
}

pub fn purge_all_roles(state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.roles.write().unwrap().clear();

    #[cfg(feature = "cache_sync")]
    state.sync.send(MapType::Roles, "".to_owned());
}

/* ------------------------------------------- User Info -------------------------------------------------- */

pub async fn request_user(user_id: Uuid, state: &AppState) -> Option<UserInfo> {
    {
        let users = state.cache.users.read().unwrap();

        // Check if user is present in cache

        if let Some(v) = users.get(&user_id) {
            return Some(v.clone());
        }
    }

    // Otherwise, fetch user from DB and insert them into the cache

    let mut con = state.db.begin().await.unwrap();

    let u = user_db::get_user_info(user_id, con.as_mut()).await;

    let _ = con.commit().await;

    if u.is_err() {
        return None;
    }

    {
        let user = u.unwrap();

        let mut users = state.cache.users.write().unwrap();

        users.insert(user_id, user.clone());

        Some(user)
    }
}

pub fn purge_user(user_id: Uuid, state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.users.write().unwrap().remove(&user_id);

    #[cfg(feature = "cache_sync")]
    state.sync.send(MapType::Users, user_id.to_string());
}

pub fn purge_all_users(state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.users.write().unwrap().clear();

    #[cfg(feature = "cache_sync")]
    state.sync.send(MapType::Users, "".to_owned());
}

/* --------------------------------------------- Sensors ----------------------------------------------------- */

pub async fn request_sensor(sensor_id: Uuid, state: &AppState) -> Option<FullSensorInfo> {
    {
        let sensors = state.cache.sensors.read().unwrap();

        // Check if sensor is present in cache

        if let Some(v) = sensors.get(&sensor_id) {
            return Some(v.clone());
        }
    }

    // Otherwise, fetch sensor from DB and insert them into the cache

    let mut con = state.db.begin().await.unwrap();

    let sp = sensor_db::get_full_sensor_info(sensor_id, con.as_mut()).await;

    let _ = con.commit().await;

    if sp.is_err() {
        return None;
    }

    {
        let sp = sp.unwrap();

        let mut sensors = state.cache.sensors.write().unwrap();

        sensors.insert(sensor_id, sp.clone());

        Some(sp)
    }
}

pub fn purge_sensor(sensor_id: Uuid, state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.sensors.write().unwrap().remove(&sensor_id);

    #[cfg(feature = "cache_sync")]
    state.sync.send(
        MapType::Sensors,
        serde_json::to_string(&cache_sync::SensorPurgeEvent {
            is_sensor_id: true,
            key: sensor_id,
        })
        .unwrap(),
    );
}

pub fn purge_sensors_owned_by(user_id: Uuid, state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state
        .cache
        .sensors
        .write()
        .unwrap()
        .retain(|_, s| s.owner.is_none() || s.owner.unwrap() != user_id);

    #[cfg(feature = "cache_sync")]
    state.sync.send(
        MapType::Sensors,
        serde_json::to_string(&cache_sync::SensorPurgeEvent {
            is_sensor_id: false,
            key: user_id,
        })
        .unwrap(),
    );
}

pub fn purge_all_sensors(state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.sensors.write().unwrap().clear();

    #[cfg(feature = "cache_sync")]
    state.sync.send(MapType::Sensors, "".to_owned());
}

/* ----------------------------------------------- API Keys -------------------------------------------------- */

pub async fn request_api_key(key_id: Uuid, state: &AppState) -> Option<ApiKey> {
    {
        let keys = state.cache.api_keys.read().unwrap();

        // Check if key is present in cache

        if let Some(v) = keys.get(&key_id) {
            return Some(v.clone());
        }
    }

    // Otherwise, fetch key from DB and insert them into the cache

    let k = sensor_db::get_api_key(key_id, &state).await;

    if k.is_err() {
        return None;
    }

    {
        let key = k.unwrap();

        let mut keys = state.cache.api_keys.write().unwrap();

        keys.insert(key_id, key.clone());

        Some(key)
    }
}

pub fn purge_api_key(key_id: Uuid, state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.api_keys.write().unwrap().remove(&key_id);

    #[cfg(feature = "cache_sync")]
    state.sync.send(
        MapType::ApiKeys,
        serde_json::to_string(&cache_sync::ApiKeyPurgeEvent {
            is_apikey: true,
            is_sensor: false,
            is_user: false,
            key: key_id,
        })
        .unwrap(),
    );
}

pub fn purge_api_keys_for_sensor(sensor_id: Uuid, state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state
        .cache
        .api_keys
        .write()
        .unwrap()
        .retain(|_, v| v.sensor_id != sensor_id);

    #[cfg(feature = "cache_sync")]
    state.sync.send(
        MapType::ApiKeys,
        serde_json::to_string(&cache_sync::ApiKeyPurgeEvent {
            is_apikey: false,
            is_sensor: true,
            is_user: false,
            key: sensor_id,
        })
        .unwrap(),
    );
}

pub fn purge_api_keys_for_user(user_id: Uuid, state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state
        .cache
        .api_keys
        .write()
        .unwrap()
        .retain(|_, v| v.user_id != user_id);

    #[cfg(feature = "cache_sync")]
    state.sync.send(
        MapType::ApiKeys,
        serde_json::to_string(&cache_sync::ApiKeyPurgeEvent {
            is_apikey: false,
            is_sensor: false,
            is_user: true,
            key: user_id,
        })
        .unwrap(),
    );
}

pub fn purge_all_keys(state: &AppState) {
    #[cfg(not(feature = "cache_sync"))]
    state.cache.api_keys.write().unwrap().clear();

    #[cfg(feature = "cache_sync")]
    state.sync.send(MapType::ApiKeys, "".to_owned());
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{database::models::role::ROLE_SYSTEM_GUEST, test_utils::tests::create_test_app};
    use sqlx::PgPool;

    #[sqlx::test(
        migrations = "../migrations",
        fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql")
    )]
    async fn test_role_purge(pool: PgPool) {
        let (_, state) = create_test_app(pool).await;

        let k = ROLE_SYSTEM_GUEST;

        // --- bring the role into the cache
        request_role(k, &state).await;

        // it should now be in the cache
        {
            let r = state.cache.roles.read().unwrap();
            assert!(r.contains_key(&k));
        }

        // remove it from the cache
        purge_role(k, &state);

        // Make sure the cache entry has really been purged
        {
            let r = state.cache.roles.read().unwrap();
            assert!(!r.contains_key(&k));
        }
    }
}
