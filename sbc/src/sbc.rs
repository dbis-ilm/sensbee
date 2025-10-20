use anyhow::anyhow;
use clap::Parser;
use reqwest::Client;
use rustyline::error::ReadlineError;
use rustyline::{Result};
use rustyline::highlight::MatchingBracketHighlighter;
use rustyline::validate::MatchingBracketValidator;
use rustyline::{Cmd, Editor, EventHandler, KeyCode, KeyEvent, Modifiers};
use rustyline::{Completer, Helper, Highlighter, Hinter, Validator};
use serde_json::json;
use colored::Colorize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use sbc::help::*;
use sbc::sensor_cmd::handle_sensor_cmd;
use sbc::ingest_cmd::handle_ingest_cmd;
use sbc::user_cmd::{handle_user_cmd};
use sbc::role_cmd::handle_role_cmd;
use sbc::data_cmd::handle_data_cmd;
use sbc::ollama::handle_ask_cmd;
use sbc::me::init_me;

#[derive(Completer, Helper, Highlighter, Hinter, Validator)]
struct InputValidator {
    #[rustyline(Validator)]
    brackets: MatchingBracketValidator,
    #[rustyline(Highlighter)]
    highlighter: MatchingBracketHighlighter,
}


async fn handle_cmd(cmd: &str, url: &str, jwt_token: &str) -> anyhow::Result<()> {
    let s = cmd.replace("\n", " ");
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts[0] == "help" {
        print_help_message();
    } else if parts.len() < 2 {
        println!("Error: invalid command");
    } else {
        match parts[0] {
            "user" => { let _ = handle_user_cmd(parts, url, jwt_token).await; }
            "role" => { let _ = handle_role_cmd(parts, url, jwt_token).await; }
            "sensor" => { let _ = handle_sensor_cmd(parts, url, jwt_token).await; }
            "ingest" => { let _ = handle_ingest_cmd(parts, url).await; }
            "data" => { let _ = handle_data_cmd(parts, url).await; }
            _ => {
                println!("unknown command {}", parts[0]);
            }
        }
    }
    Ok(())
}

async fn read_eval_loop(url: &str, jwt_token: &str) -> Result<()> {
    print_welcome_message();
      let h = InputValidator {
        brackets: MatchingBracketValidator::new(),
        highlighter: MatchingBracketHighlighter::new(),
    };
    let mut rl = Editor::new()?;
    rl.set_helper(Some(h));
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('x'), Modifiers::CTRL),
        EventHandler::Simple(Cmd::AcceptOrInsertLine {
                accept_in_the_middle: true,
            }),
    );
    rl.bind_sequence(
         KeyEvent(KeyCode::Enter, Modifiers::NONE),
        EventHandler::Simple(Cmd::Newline),
    );
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    let prompt = format!("{}@{}> ", "sbc".bright_blue(), url.yellow());
    loop {
        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                if line.starts_with("ask") {
                    let res = handle_ask_cmd(&line[4..]).await;  
                    match res {
                        Ok(cmd) => { let _ = rl.add_history_entry(cmd); }
                        Err(_) => { println!("ERROR: cannot answer question."); }
                    }
                }   
                else {
                    let _ = handle_cmd(&line, url, jwt_token).await;
                }
                         
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("bye!");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    let _ = rl.save_history("history.txt");
    Ok(())
}

async fn process_command_file(url: &str, jwt_token: &str, file_name: &str) -> Result<()> {
    let file = File::open(file_name)?;
    let reader = BufReader::new(file);

     let mut buffer = String::new();
    for line in reader.lines() {
        let l = line?;
        buffer.push_str(&l);

        while let Some(idx) = buffer.find(';') {
            let (cmd, rest) = buffer.split_at(idx);
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                println!("Execute: {}", cmd);
                let _ = handle_cmd(cmd, url, jwt_token).await;
            }
            buffer = rest[1..].to_string(); // Semikolon entfernen
        }
    }
    let cmd = buffer.trim();
    if !cmd.is_empty() {
        println!("Execute: {}", cmd);
        let _ = handle_cmd(cmd, url, jwt_token).await;
    }
    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// URL of SensBee server
    #[arg(short, long)]
    server: String,

    /// User for login
    #[arg(short, long)]
    user: String,

    // Password of user
    #[arg(short, long)]
    password: Option<String>,

    /// File for script
    #[arg(short, long)]
    file: Option<String>,
}

async fn connect(client: Client, url: &str, login: &str, passwd: &str) -> anyhow::Result<String> {
    let payload = json!({
        "email": login,
        "password": passwd
    });
    let res = client
        .post(format!("{}/auth/login", url))
        .json(&payload)
        .send()
        .await?;
    // extract JWT token
    if let Some(cookie) = res.headers().get("set-cookie") {
        let parts = cookie.to_str().expect("Invalid cookie").split(";");
        for p in parts {
            if p.starts_with("token=") {
                let jwt_token = &p[6..];
                return Ok(jwt_token.to_string());
            }
        }
    }
    Err(anyhow!("Connection failed"))
}


#[tokio::main]
async fn main() -> anyhow::Result<(), reqwest::Error> {
    let args = Args::parse();
    let mut jwt_token: String = "".to_string();

    let client = Client::new();

    if let Some(password) = args.password.as_deref() {
        // try to connect
        let resp = connect(client, &args.server, &args.user, password).await;
        match resp {
            Ok(token) => jwt_token = token,
            Err(err) => {
                println!("ERROR: {}", err);
                std::process::exit(-1);
            }
        }
    } else {
        let password = rpassword::prompt_password("Your password: ").unwrap();
        let _ = connect(client, &args.server, &args.user, &password).await;
    }

    let _ = init_me(&args.server, &jwt_token, &args.user).await;

    if let Some(file) = args.file.as_deref() {
        let _ = process_command_file(&args.server, &jwt_token, file).await;
    } else {
        let _ = read_eval_loop(&args.server, &jwt_token).await;
    }
    Ok(())
}
