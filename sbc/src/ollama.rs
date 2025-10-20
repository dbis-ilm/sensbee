use anyhow::Result;
use ollama_rs::IntoUrlSealed;
use reqwest::{Client,Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::env;

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    response: String,
    done: bool,
}

pub struct OllamaClient {
    client: Client,
    base_url: String,
}

impl OllamaClient {
    /// Creates a new Ollama client with optimized settings
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client, base_url }
    }

    /// Sends a prompt to Ollama and returns the response
    /// Memory-safe: Automatic cleanup of request/response objects
    pub async fn generate(&self, model: &str, prompt: &str) -> Result<String> {
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let response = self
            .client
            .post(&format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        let ollama_response: OllamaResponse = response.json().await?;
        Ok(ollama_response.response)
    }
}

#[derive(Debug, Deserialize)]
pub struct OllamaConfig {
    pub model: String,
    pub ollama_url: String,
    pub max_concurrent_requests: usize,
    pub request_timeout_seconds: u64,
    pub retry_attempts: u32,
}

impl OllamaConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            model: env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "gemma3:4b-it-qat".to_string()),
            ollama_url: env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            max_concurrent_requests: env::var("MAX_CONCURRENT_REQUESTS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()?,
            request_timeout_seconds: env::var("REQUEST_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
            retry_attempts: env::var("RETRY_ATTEMPTS")
                .unwrap_or_else(|_| "3".to_string())
                .parse()?,
        })
    }
}

async fn ask(request: &str) -> Result<String> {
    // Load configuration
    let config = OllamaConfig::from_env()?;
    let client = OllamaClient::new(config.ollama_url.clone());

    let prompt = request;
    let res = client.generate(&config.model, prompt).await?;
    println!("{}", res);
    Ok(res)
}

pub async fn handle_ask_cmd(cmd: &str) -> anyhow::Result<String> {
let prompt = r#"
Ein Sensor wird mit folgender Anweisung erzeugt:
sensor create {"columns":[{"name":"count","val_type":"INT","val_unit":"number","val_ingest":"LITERAL"},
{"name":"temperature","val_type":"FLOAT","val_unit":"celsius","val_ingest":"LITERAL"}],
"description":"This is my first sensor.","name":"MySensor",
"position":[50.68322,10.91858]}

Zeige eine Anweisung für folgenden Sensor:
"#;

let prompt2 = r#"
Gib keine Erläuterungen und zeige nur die Anweisung.
"#;
    let res = ask(format!("{} {} {}", prompt, cmd, prompt2).as_str()).await;
    res
}
