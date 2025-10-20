use anyhow::anyhow;
use serde_json::{Value};
use comfy_table::Table;
use reqwest::Client;
use crate::help::*;
use crate::me::get_read_api_key;

fn print_table(json: &Value) {
    let mut header = false;
    let mut table = Table::new();
    let array = json.as_array().unwrap();
    for obj in array {
        if !obj.is_object() {
            println!("ERROR: expected an object, but got: {:?}", obj);
            return;
        }
        let o = obj.as_object().unwrap();
        if !header {
            let headers: Vec<String> = o.keys().cloned().collect();
            table.set_header(headers);
            header = true;
        }
        let data = o.values().map(|v| v.to_string()).collect::<Vec<String>>();
        table.add_row(data);
    }

    println!("{}", table);
}

pub async fn handle_data_cmd(cmd: Vec<&str>, url: &str) -> anyhow::Result<()> {
    if cmd[1] == "help" {
        print_data_help_message();
        return Ok(())  
    }
    if cmd.len() < 3 {
       return Err(anyhow!("Invalid command"));    
    }
    let sensor_id = cmd[2];

    let api_key = get_read_api_key(sensor_id).await;
    if api_key.is_none() {
        println!("missing API key for read"); 
        return Err(anyhow!("Missing API key"));  
    } 
    match cmd[1] {
        "show" => { /* just continue */ }
        "help" => { print_data_help_message();
            return Ok(())
        }
        _ => {
            println!("unknown subcommand '{}'", cmd[1]); 
            return Err(anyhow!("Invalid command"));            
        }
    }
    let client = Client::new();

    let res = client
        .get(format!("{}/api/sensors/{}/data/load?key={}&limit=1000", url, sensor_id, api_key.unwrap()))    
        .send()
        .await?;
    let json: Value = res.json().await.unwrap();
    print_table(&json);
    /* 
    let table = json_to_table(&json).collapse().object_orientation(Orientation::Row).to_string();

    println!("{}", table);
    */

    Ok(())
}
