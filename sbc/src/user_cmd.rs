use anyhow::anyhow;
use serde_json::Value;
use json_to_table::{json_to_table, Orientation};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;


use crate::{help::*, not_implemented};
use crate::me::user_me;

fn user_create(cmd: Vec<&str>) {
    // TODO
    println!("create user: {}", cmd[2])
}

async fn user_list(url: &str, jwt_token: &str) -> anyhow::Result<()> {
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
    let table = json_to_table(&value).object_orientation(Orientation::Row).to_string();
    println!("{}", table);
    Ok(())
}

async fn user_info(url: &str, jwt_token: &str, user_id: &str) -> anyhow::Result<()> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(jwt_token).unwrap());
    let client = Client::new();
    let res = client
        .get(format!("{}/api/users/{}/info", url, user_id))
        .headers(headers)
        .send()
        .await?;
       let value: Value = res.json().await.unwrap();
    let table = json_to_table(&value).collapse().to_string();
    println!("{}", table);
    Ok(())
}

pub async fn handle_user_cmd(cmd: Vec<&str>, url: &str, jwt_token: &str) -> anyhow::Result<()> {
    match cmd[1] {
        "me" => { let _ = user_me(url, jwt_token).await; }
        "create" => user_create(cmd),
        "delete" => { not_implemented!("user delete") }
        "list" => { let _ = user_list(url, jwt_token).await; }
        "info" => { 
            if cmd.len() < 3 {
                println!("user_id missing");
                return Err(anyhow!("Invalid command"));
            }
            let _ = user_info(url, jwt_token, cmd[2]).await; }
        "password" => { not_implemented!("user password") }
        "help" => print_user_help_message(),
         _ => { println!("unknown subcommand '{}'", cmd[1]);
                return Err(anyhow!("Invalid command"));            
        }
    }
    Ok(())
}
