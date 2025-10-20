use anyhow::anyhow;
use serde_json::{Value};
use reqwest::Client;
use csv::Reader;
use crate::help::*;
use crate::me::get_write_api_key;

async fn load_from_file(filename: &str) -> anyhow::Result<Value> {
    if filename.ends_with(".json") {
        let content = tokio::fs::read_to_string(filename).await?;
        let value: Value = serde_json::from_str(&content)?;
        Ok(value)
    } else if filename.ends_with(".csv") {
        let content = tokio::fs::read_to_string(filename).await?;
        let mut rdr = Reader::from_reader(content.as_bytes());
        let headers = rdr.headers()?.clone();
        let mut records = Vec::new();
        for result in rdr.records() {
            let record = result?;
            // Wandelt jede Zeile in ein HashMap um: Spaltenname -> Wert
            let mut map = serde_json::Map::new();
            for (header, field) in headers.iter().zip(record.iter()) {
                // Versuche, das Feld als Zahl zu parsen
                if let Ok(n) = field.parse::<i64>() {
                    map.insert(header.to_string(), Value::Number(n.into()));
                } else if let Ok(f) = field.parse::<f64>() {
                    // f64 muss überNumber::from_f64 erzeugt werden
                    if let Some(num) = serde_json::Number::from_f64(f) {
                        map.insert(header.to_string(), Value::Number(num));
                    } else {
                        map.insert(header.to_string(), Value::String(field.to_string()));
                    }
                } else {
                    map.insert(header.to_string(), Value::String(field.to_string()));
                }
            }
            records.push(Value::Object(map));
        }
        // println!("records: {:?}", records);
        let value = Value::Array(records);
        println!("value: {:?}", value);
        Ok(value)
    } else {
        Err(anyhow!("File must be a JSON or CSV file"))
    }
}

pub async fn handle_ingest_cmd(cmd: Vec<&str>, url: &str) -> anyhow::Result<()> {
    if cmd[1] == "help" {
        print_ingest_help_message();
        return Ok(())  
    }
    if cmd.len() < 4 {
       return Err(anyhow!("Invalid command"));    
    }
    let sensor_id = cmd[1];
    let api_key = get_write_api_key(sensor_id).await;
    if api_key.is_none() {
        // let _ = create_api_key(url, jwt_token, sensor_id, "WRITE").await;
        println!("missing API key for writes"); 
        return Err(anyhow!("Missing API key"));  
    }
    let mut value: Value = Value::Null;
    match cmd[2] {
        "file" => {
            let filename = cmd[3].trim().trim_matches('\'');
            value = load_from_file(filename).await?;
        }
        "json" => {
            let str = &cmd[3..].concat().to_string();
            value = serde_json::from_str(str)?;
        }
        "help" => { print_ingest_help_message();
            return Ok(())
        }
        _ => {
            println!("unknown subcommand '{}'", cmd[1]); 
            return Err(anyhow!("Invalid command"));            
        }
    }
    let client = Client::new();

    println!("ingest value: {:?}", value);
    let res = client
        .post(format!("{}/api/sensors/{}/data/ingest?key={}", url, sensor_id, api_key.unwrap()))    
        .json(&value)
        .send()
        .await?;
    let status = res.status();
    let json: Value = res.json().await.unwrap();
    println!("response {:?} -> {:?}", status, json);

    Ok(())
}
