use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::sync::RwLock;
use std::collections::HashMap;
use json_to_table::json_to_table;
use anyhow::anyhow;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;

use crate::sensor_cmd::get_sensor_info;
use crate::role_cmd::role_request;

#[derive(Clone)]
pub struct Me {
    url: Option<String>,
    jwt_token: Option<String>,
    email: Option<String>,
    user_id: Option<String>,
    roles: HashMap<String, String>,
    read_api_keys: HashMap<String, String>,
    write_api_keys: HashMap<String, String>,
}

static ME: Lazy<RwLock<Me>> = 
    Lazy::new(|| RwLock::new(Me {
        url: None,
        jwt_token: None,
        email: None, 
        user_id: None,
        roles: HashMap::new(),
        read_api_keys: HashMap::new(),
        write_api_keys: HashMap::new(),
    } ));

pub async fn init_me(url: &str, jwt_token: &str, email: &str) -> anyhow::Result<()> {
    let mut me = ME.write().unwrap();
    (*me).url = Some(url.to_string());
    (*me).jwt_token = Some(jwt_token.to_string());
    if email.len() > 0 {
        (*me).email = Some(email.to_string());
    }
    let res = user_me_request(url, jwt_token, email).await;
    match res {
        Ok(val) => {
            if let Some(user_id) = val["id"].as_str() {
                (*me).user_id = Some(user_id.to_string());
            }
        }
        Err(_) => {
            println!("ERROR: user_me_request failed!")
        }
    }
    let res = role_request(url, jwt_token).await;
    match res {
        Ok(val) => {
            if let Some(arr) = val.as_array() {
                for role in arr {
                    let r_name = role["name"].as_str();
                    let r_id = role["id"].as_str();
                    (*me).roles.insert(r_name.unwrap().to_string(), r_id.unwrap().to_string());
                }
                drop(me);
                return Ok(())
            }
        }
        Err(_) => {
            println!("Cannot obtain roles");
        }
    }
    drop(me);
    Err(anyhow!("Cannot initialize ME"))
}

pub fn get_me_ref() -> std::sync::RwLockReadGuard<'static, Me> {
    ME.read().unwrap()
}


pub fn get_role_id(role: &str) -> Option<String> {
    let me = ME.read().unwrap();
    let res = (*me).roles.get(role);
    res.cloned()    
}   

pub async fn get_read_api_key(sensor_id: &str) -> Option<String> {
    let mut me = ME.read().unwrap();
    let mut key = (*me).read_api_keys.get(sensor_id);
    if key.is_none() {
        drop(me);
        let _ = fetch_read_api_key(sensor_id).await;
        me = ME.read().unwrap();
        key = (*me).read_api_keys.get(sensor_id);
    }
    key.cloned()
}


pub async fn get_write_api_key(sensor_id: &str) -> Option<String> {
    let mut me = ME.read().unwrap();
    let mut key = (*me).write_api_keys.get(sensor_id);
    if key.is_none() {
        drop(me);
        let _ = fetch_write_api_key(sensor_id).await;
        me = ME.read().unwrap();
        key = (*me).write_api_keys.get(sensor_id);
    }
    key.cloned()
}

async fn fetch_read_api_key(sensor_id: &str) -> anyhow::Result<()> {
    let mut me = ME.write().unwrap();
    let value = get_sensor_info((*me).url.as_ref().unwrap(), (*me).jwt_token.as_ref().unwrap(), sensor_id).await?;
    let keys = value.as_object().unwrap()["api_keys"].as_array().unwrap();
    for keyv in keys {
        let user_id = (*me).user_id.as_ref().unwrap();
        if keyv["user_id"].as_str().unwrap() == user_id && keyv["operation"] == "READ" {
            let key = keyv["id"].as_str().unwrap();
            (*me).read_api_keys.insert(sensor_id.to_string(), key.to_string());
            return Ok(())
        }
    }
    Err(anyhow!("Cannot obtain read API key for sensor"))
}

async fn fetch_write_api_key(sensor_id: &str) -> anyhow::Result<()> {
    let mut me = ME.write().unwrap();
    let value = get_sensor_info((*me).url.as_ref().unwrap(), (*me).jwt_token.as_ref().unwrap(), sensor_id).await?;
    let keys = value.as_object().unwrap()["api_keys"].as_array().unwrap();
    for keyv in keys {
        let user_id = (*me).user_id.as_ref().unwrap();
        if keyv["user_id"].as_str().unwrap() == user_id && keyv["operation"] == "WRITE" {
            let key = keyv["id"].as_str().unwrap();
            (*me).write_api_keys.insert(sensor_id.to_string(), key.to_string());
            drop(me);
            return Ok(())
        }
    }
    Err(anyhow!("Cannot obtain write API key for sensor"))
}

fn add_read_api_key(sensor_id: &str, key: &str) {
    let mut me = ME.write().unwrap();
    (*me).read_api_keys.insert(sensor_id.to_string(), key.to_string());
    drop(me);
}


fn add_write_api_key(sensor_id: &str, key: &str) {
    let mut me = ME.write().unwrap();
    (*me).write_api_keys.insert(sensor_id.to_string(), key.to_string());
    drop(me);
}


pub async fn create_api_key(url: &str, jwt_token: &str, sensor_id: &str, operation: &str) -> anyhow::Result<()> {
    let value = json!({
        "name": "MyKey",
        "operation": operation,
    });
   let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .post(format!("{}/api/sensors/{}/api_key/create", url, sensor_id))    
        .headers(headers)
        .json(&value)
        .send()
        .await?;
    let json: Value = res.json().await.unwrap();
    if let Some(key) = json.as_object() {
        let key_id = key["id"].as_str().unwrap();
        match operation {
            "WRITE" => add_write_api_key(sensor_id, key_id),
            "READ" => add_read_api_key(sensor_id, key_id),
            _ => return Err(anyhow!("Invalid operation: only READ or WRITE are expected"))
        };
        return Ok(())
    }
    Err(anyhow!("Cannot obtain API key"))
}


async fn user_me_request(url: &str, jwt_token: &str, email: &str) -> anyhow::Result<Value> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .get(format!("{}/api/users/list", url))
        .headers(headers)
        .send()
        .await?;
    let value: Value = res.json().await.unwrap();
    //let email = me.email.unwrap();
    if let Some(arr) = value.as_array() {
        for user in arr {
            if let Some(u) = user.as_object() {
                if u["email"] == email {
                    return Ok(user.clone());
                }
            }
        }
    }
    Err(anyhow!("Cannot obtain user list"))
}

pub async fn user_me(url: &str, jwt_token: &str) -> anyhow::Result<()> {
    let me = get_me_ref();
    let email = &me.email.as_ref().unwrap();
    let res = user_me_request(url, jwt_token, &email).await;
    match res {
        Ok(val) => {
            let table = json_to_table(&val).collapse().to_string();
            println!("{}", table);
            Ok(())
        }
        Err(msg) => {
            return Err(msg)
        }
    }
}
