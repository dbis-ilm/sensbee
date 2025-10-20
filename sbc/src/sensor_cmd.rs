use anyhow::anyhow;
use serde_json::{json, Value};
use json_to_table::{json_to_table, Orientation};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use crate::help::*;
use crate::me::get_role_id;

async fn sensor_create(url: &str, jwt_token: &str, cmd: &str) -> anyhow::Result<()> {
    let user_role = get_role_id("User");
    
    let mut value: Value = serde_json::from_str(cmd)?;
    
    // add permissions if missing
    if value["permissions"].is_null() {
        if let Some(obj) = value.as_object_mut() {
            let permissions = json!([{
                "role_id": user_role,
                "operations": ["INFO", "READ", "WRITE"]
            }]);
            obj.insert("permissions".to_string(), permissions);
        }
    }
    // add storage if missing
    if value["storage"].is_null() {
        if let Some(obj) = value.as_object_mut() {
            let storage = json!({
                "variant": "DEFAULT",
                "params": {}
            });
            obj.insert("storage".to_string(), storage);
        }
    }
    println!("value = {}", value);
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .post(format!("{}/api/sensors/create", url))    
        .headers(headers)
        .json(&value)
        .send()
        .await?;
    let json: Value = res.json().await.unwrap();
    println!("json: {:?}", json);

    Ok(())
}

pub async fn get_sensor_info(url: &str, jwt_token: &str, sensor_id: &str) -> anyhow::Result<Value> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .get(format!("{}/api/sensors/{}/info", url, sensor_id))
        .headers(headers)
        .send()
        .await?;
    let value: Value = res.json().await.unwrap();
    Ok(value)
}

async fn sensor_info(url: &str, jwt_token: &str, sensor_id: &str) -> anyhow::Result<()> {
    let value = get_sensor_info(url, jwt_token, sensor_id).await?;
    let table = json_to_table(&value).collapse().to_string();
    println!("{}", table);
    Ok(())
}

async fn sensor_delete(url: &str, jwt_token: &str, sensor_id: &str) -> anyhow::Result<()> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .delete(format!("{}/api/sensors/{}/delete", url, sensor_id))
        .headers(headers)
        .send()
        .await?;
    let json: Value = res.json().await.unwrap();
    println!("json: {:?}", json);
    // let data = res.json();
    Ok(())
}

async fn sensor_list(url: &str, jwt_token: &str) -> anyhow::Result<()> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .get(format!("{}/api/sensors/list", url))
        .headers(headers)
        .send()
        .await?;
    let value: Value = res.json().await.unwrap();
    let table = json_to_table(&value).object_orientation(Orientation::Row).to_string();
    println!("{}", table);
    Ok(())
}

async fn sensor_delete_key(url: &str, jwt_token: &str, sensor_id: &str, key_id: &str) -> anyhow::Result<()> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .delete(format!("{}/api/sensors/{}/api_key/{}/delete", url, sensor_id, key_id))
        .headers(headers)
        .send()
        .await?;
    let json: Value = res.json().await.unwrap();
    println!("json: {:?}", json);
    // TODO: delete from ME
    Ok(())
}

async fn sensor_create_read_key(url: &str, jwt_token: &str, sensor_id: &str, key_name: &str) -> anyhow::Result<()> {
    let value = json!({
        "name": key_name,
        "operation": "READ",
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
        println!("created read key: {}", key["key"]);
    }
        // TODO: add to ME
    Ok(())
}

async fn sensor_create_write_key(url: &str, jwt_token: &str, sensor_id: &str, key_name: &str) -> anyhow::Result<()> {
    let value = json!({
        "name": key_name,
        "operation": "WRITE",
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
        println!("created write key: {}", key["key"]);
    }
            // TODO: add to ME
    Ok(())
}

pub async fn handle_sensor_cmd(cmd: Vec<&str>, url: &str, jwt_token: &str) -> anyhow::Result<()> {
    match cmd[1] {
        "create" => {
            if cmd.len() < 3 {
                println!("sensor definition missing");
                return Err(anyhow!("Invalid command"));
            }
            let _ = sensor_create(url, jwt_token, &cmd[2..].concat().to_string()).await;
        }
        "delete" => {
                    if cmd.len() < 3 {
                        println!("sensor_id missing");
                        return Err(anyhow!("Invalid command"));
                    }
                    let _ = sensor_delete(url, jwt_token, cmd[2]).await;
        }
        "info" => {
            if cmd.len() < 3 {
                        println!("sensor_id missing");
                        return Err(anyhow!("Invalid command"));
            }
            let _ = sensor_info(url, jwt_token, cmd[2]).await;
        }
        "list" => {
            let _ = sensor_list(url, jwt_token).await;
        }
        "delete_key" => {
            if cmd.len() < 4 {
                println!("sensor id or key name missing");
                return Err(anyhow!("Invalid command"));
            }
            let _ = sensor_delete_key(url, jwt_token, cmd[2], cmd[3]).await;
        },
        "create_read_key" => {
            if cmd.len() < 4 {
                println!("sensor id or key name missing");
                return Err(anyhow!("Invalid command"));
            }
            let _ = sensor_create_read_key(url, jwt_token, cmd[2], cmd[3]).await;
        },
        "create_write_key" => {
            if cmd.len() < 4 {
                println!("sensor id or key name missing");
                return Err(anyhow!("Invalid command"));
            }
            let _ = sensor_create_write_key(url, jwt_token, cmd[2], cmd[3]).await;
        },
        "help" => print_sensor_help_message(),
        _ => {
            println!("unknown subcommand '{}'", cmd[1]);
            return Err(anyhow!("Invalid command"));            
        }
    }
    Ok(())
}
