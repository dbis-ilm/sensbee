use anyhow::anyhow;
use serde_json::{Value};
use json_to_table::{json_to_table, Orientation};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use crate::{help::*, not_implemented};

pub async fn role_request(url: &str, jwt_token: &str) -> anyhow::Result<Value> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .get(format!("{}/api/roles/list", url))
        .headers(headers)
        .send()
        .await;
    match res {
        Ok(res) => {
            let value: Value = res.json().await.unwrap();
            Ok(value)
        }
        Err(_) => {
            Err(anyhow!("Cannot obtain role list"))    
        }
    }
}

async fn role_list(url: &str, jwt_token: &str) -> anyhow::Result<()> {
    let res = role_request(url, jwt_token).await;
    match res {
        Ok(value) => {
            let table = json_to_table(&value).object_orientation(Orientation::Row).to_string();
            println!("{}", table);
            Ok(())
        }
        Err(msg) => { Err(msg) }
    }
}


pub async fn handle_role_cmd(cmd: Vec<&str>, url: &str, jwt_token: &str) -> anyhow::Result<()> {
    match cmd[1] {
        "create" => { not_implemented!("role create") }
        "delete" => { not_implemented!("role create") }
        "list" => { let _ = role_list(url, jwt_token).await; }
        "help" => print_role_help_message(),
         _ => { println!("unknown subcommand '{}'", cmd[1]);
                return Err(anyhow!("Invalid command"));            
        }
    }
    Ok(())
}
