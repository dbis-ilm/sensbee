use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;
use once_cell::sync::Lazy;
use sqlx::postgres::any::AnyConnectionBackend;
use crate::database::models::role::Role;
use crate::database::{role_db, sensor_db, user_db};
use crate::database::models::api_key::ApiKey;
use crate::database::models::sensor::{FullSensorInfo};
use crate::database::models::user::UserInfo;
use crate::state::AppState;

// Thread-safe data cache across the application
// It must be ensured, that the locks are not held across await points, since this is not a 
// Send+Sync safe implementation for processing across different threads/tasks.

/* ------------------------------------------------------------------------------------------------- */

// TODO: We should add a test for multi-threaded accesses

static ROLES: Lazy<RwLock<HashMap<String, Role>>> = Lazy::new(RwLock::default);
static USERS: Lazy<RwLock<HashMap<Uuid, UserInfo>>> = Lazy::new(RwLock::default);
static SENSORS: Lazy<RwLock<HashMap<Uuid, FullSensorInfo>>> = Lazy::new(RwLock::default);
static API_KEYS: Lazy<RwLock<HashMap<Uuid, ApiKey>>> = Lazy::new(RwLock::default);

#[cfg(test)]
const ENABLED: bool = false;

#[cfg(not(test))]
const ENABLED: bool = true;

pub fn purge_all() {
    purge_all_users();
    purge_all_sensors();
    purge_all_keys();
    purge_all_roles();
}

/* --------------------------------------------- Roles ---------------------------------------------------- */

pub async fn request_role(role_name: String, state: &AppState) -> Option<Role> {
    {
        let roles = ROLES.read().unwrap();

        // Check if role is present in cache

        if roles.contains_key(&role_name) {
            return roles.get(&role_name).cloned();
        }
    }

    // Otherwise, fetch roles from DB and insert them into the cache

    let r = role_db::get_role_by_name(role_name.clone(), &state).await;

    if r.is_err() {
        return None;
    }

    if !ENABLED {
        return Some(r.unwrap());
    }

    let role = r.unwrap();

    let mut roles = ROLES.write().unwrap();

    roles.insert(role_name.clone(), role.clone());

    Some(role)
}

pub fn purge_role(role_name: String) {
    let mut roles = ROLES.write().unwrap();

    roles.remove(&role_name);
}

pub fn purge_all_roles() {
    let mut roles = ROLES.write().unwrap();

    roles.clear()
}

/* ------------------------------------------- User Info -------------------------------------------------- */

pub async fn request_user(user_id: Uuid, state: &AppState) -> Option<UserInfo> {
    {
        let users = USERS.read().unwrap();

        // Check if user is present in cache

        if users.contains_key(&user_id) {
            return users.get(&user_id).cloned();
        }
    }

    // Otherwise, fetch user from DB and insert them into the cache

    let mut con = state.get_db_connection().await.unwrap();

    let u = user_db::get_user_info(user_id, con.as_mut()).await;
    
    let _ = con.commit();

    if u.is_err() {
        return None;
    }

    if !ENABLED {
        return Some(u.unwrap());
    }

    let user = u.unwrap();

    let mut users = USERS.write().unwrap();

    users.insert(user_id, user.clone());

    Some(user)
}

pub fn purge_user(user_id: Uuid) {
    let mut user = USERS.write().unwrap();

    user.remove(&user_id);
}

pub fn purge_all_users() {
    let mut users = USERS.write().unwrap();

    users.clear()
}

/* --------------------------------------------- Sensors ----------------------------------------------------- */

pub async fn request_sensor(sensor_id: Uuid, state: &AppState) -> Option<FullSensorInfo> {
    {
        let sensors = SENSORS.read().unwrap();

        // Check if sensor is present in cache

        if sensors.contains_key(&sensor_id) {
            return sensors.get(&sensor_id).cloned();
        }
    }

    // Otherwise, fetch sensor from DB and insert them into the cache
    
    let mut con = state.get_db_connection().await.unwrap();

    let sp = sensor_db::get_full_sensor_info(sensor_id, con.as_mut()).await;

    let _ = con.commit();

    if sp.is_err() {
        return None;
    }

    if !ENABLED {
        return Some(sp.unwrap());
    }

    let sp = sp.unwrap();

    let mut sensors = SENSORS.write().unwrap();

    sensors.insert(sensor_id, sp.clone());

    Some(sp)
}

pub fn purge_sensor(sensor_id: Uuid) {
    let mut sensors = SENSORS.write().unwrap();

    sensors.remove(&sensor_id);
}

pub fn purge_sensors_owned_by(user_id: Uuid) {
    let mut sensors = SENSORS.write().unwrap();

    sensors.retain(|_, s| s.owner.is_none() || s.owner.unwrap() != user_id);
}

pub fn purge_all_sensors() {
    let mut sensors = SENSORS.write().unwrap();

    sensors.clear()
}

/* ----------------------------------------------- API Keys -------------------------------------------------- */

pub async fn request_api_key(key_id: Uuid, state: &AppState) -> Option<ApiKey> {
    {
        let keys = API_KEYS.read().unwrap();

        // Check if key is present in cache

        if keys.contains_key(&key_id) {
            return keys.get(&key_id).cloned();
        }
    }

    // Otherwise, fetch key from DB and insert them into the cache

    let k = sensor_db::get_api_key(key_id, &state).await;

    if k.is_err() {
        return None;
    }

    if !ENABLED {
        return Some(k.unwrap());
    }

    let key = k.unwrap();

    let mut keys = API_KEYS.write().unwrap();

    keys.insert(key_id, key.clone());

    Some(key)
}

pub fn purge_api_key(key_id: Uuid) {
    let mut keys = API_KEYS.write().unwrap();

    keys.remove(&key_id);
}

pub fn purge_api_keys_for_sensor(sensor_id: Uuid) {
    let mut keys = API_KEYS.write().unwrap();

    keys.retain(|_, v| v.sensor_id != sensor_id);
}

pub fn purge_api_keys_for_user(user_id: Uuid) {
    let mut keys = API_KEYS.write().unwrap();

    keys.retain(|_, v| v.user_id != user_id);
}

pub fn purge_all_keys() {
    let mut keys = API_KEYS.write().unwrap();

    keys.clear()
}
