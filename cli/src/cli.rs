use std::env;
use clap::{Parser, Subcommand};
use env_logger::Env;
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use anyhow::Result;
use comfy_table::Table;
use uuid::Uuid;
use once_cell::unsync::Lazy;
use sqlx::postgres::any::AnyConnectionBackend;
use sensor_mgmt::state::{AppState, JWTConfig};
use sensor_mgmt::{database::user_db as user_db};
use sensor_mgmt::database::role_db;
use sensor_mgmt::handler::models::requests::{RegisterUserRequest};

// TODO: Long-Term we should not modify the DB directly but rather call designated API paths in cli handler
//       (To avoid dealing with invalid caches or left-over sensor threads / tasks [mqtt] manually)

const CLI_ACCESS_KEY: Lazy<String> = Lazy::new(|| { env::var("CLI_ACCESS_KEY").expect("CLI_ACCESS_KEY must be provided!") });
const CLI_SERVER_PORT: Lazy<u16> = Lazy::new(|| { env::var("SERVER_CLI_PORT").expect("SERVER_CLI_PORT must be provided!").parse::<u16>().unwrap() });

#[derive(Parser)]
#[command(author = "KUS", version, about="SensBee CLI")]
struct Cli {
    /// Sets the database URL
    #[arg(short, long, value_name = "URL")]
    db_url: Option<String>,
    #[command(subcommand)]
    cmd: Commands
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    // Users
    
    AddUser {
        name: String,
        email: String,
        password: String,
        #[arg(short, long)]
        admin: bool,
    },
    
    ListUsers,

    DeleteUser {
        id: Uuid,
    },

    // Roles
    
    CreateRole {
        name: String,
    },

    ListRoles,

    DeleteRole {
        name: String,
    },

    AssignRole {
        role_name: String,
        user_id: Uuid,
    },

    RevokeRole {
        role_name: String,
        user_id: Uuid,
    }
}

// --------------------------------------------- Users ---------------------------------------------

async fn add_user(name: String, email: String, password: String, admin: bool, state: &AppState) -> Result<()> {
    let user_info = RegisterUserRequest {
        name,
        email: email.clone(),
        password,
    };
    
    let res = user_db::register_user(user_info, admin, state).await?;
    let _ = user_db::verify_user(res.id, &state).await;

    println!("Created user with ID {}!", res.id.to_string());

    Ok(())
}

async fn list_users(state: &AppState) -> Result<()> {
    let mut table = Table::new();
    table.set_header(vec!["Name", "Email", "Verified", "ID", "Roles"]);

    let info = user_db::user_list(&state).await?;

    let mut con = state.get_db_connection().await?;
    
    for user in info {
        let roles = role_db::get_user_roles(user.id, con.as_mut()).await?;

        let roles: Vec<String> = roles.into_iter().map(|r| r.name).collect();
        
        table.add_row(vec![user.name, user.email, user.verified.to_string(), user.id.to_string(), roles.join(",")]);
    }
    
    let _ = con.commit();
    
    println!("{table}");

    Ok(())
}

async fn delete_user(id: Uuid, state: &AppState) -> Result<()> {
    let u = user_db::get_user_by_id(id, &state).await?;

    user_db::delete_user(u.id, state).await?;

    notify_cache_clear();

    Ok(())
}

// --------------------------------------------- Roles ---------------------------------------------

async fn create_role(role_name: String, state: &AppState) -> Result<()> {
    let res = role_db::create_role(role_name, &state).await;

    res.map(|_| ())
}

async fn list_roles(state: &AppState) -> Result<()> {
    let roles = role_db::list_roles(state).await?;

    let mut table = Table::new();
    table.set_header(vec!["Name", "ID", "System"]);

    for role in roles {
        table.add_row(vec![role.name, role.id.to_string(), role.system.to_string()]);
    }

    println!("{table}");
    
    Ok(())
}

async fn delete_role(role_name: String, state: &AppState) -> Result<()> {
    role_db::delete_role(role_name, true, &state).await?;

    notify_cache_clear();

    Ok(())
}

async fn assign_role(role_name: String, user_id: Uuid, state: &AppState) -> Result<()> {
    role_db::assign_role_by_name(user_id, role_name, true,  &state).await?;

    notify_cache_clear();

    Ok(())
}

async fn revoke_role(role_name: String, user_id: Uuid, state: &AppState) -> Result<()> {
    let _ = role_db::revoke_role(user_id, role_name, true, &state).await?;

    notify_cache_clear();

    Ok(())
}

// -------------------------------------------------------------------------------------------------

fn parse_result<T>(res: Result<T>) {
    match res {
        Ok(_) => println!("Ok!"),
        Err(_) => println!("Error during executing the command!")
    }
}

fn notify_cache_clear() -> bool {
    let client = reqwest::blocking::Client::new();

    let resp = client.post( format!("http://127.0.0.1:{}/cli/clear_cache?key={}", *CLI_SERVER_PORT, *CLI_ACCESS_KEY)).send();

    if resp.is_err() {
        println!("INFO: Couldn't connect to the REST server!");
    }

    resp.is_ok()
}

#[async_std::main]
async fn main() {
    let cli = Cli::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    dotenv().ok();
    
    let database_url = env::var("DATABASE_URL")
        .expect("Env var DATABASE_URL is required.");

    let pool = match PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url).await
    {
        Ok(pool) => {
            println!("✅ Connection to the database is successful!");
            pool
        }
        Err(err) => {
            println!("🔥 Failed to connect to the database: {:?}", err);
            std::process::exit(1);
        }
    };
    
    let state = AppState{ db: pool.clone(), jwt: JWTConfig::init() };

    match cli.cmd {
        Commands::AddUser { name, email, password, admin  } => {
            parse_result(add_user(name, email, password, admin, &state).await); 
        }
        Commands::ListUsers => { parse_result(list_users(&state).await); }
        Commands::DeleteUser { id } => { parse_result(delete_user(id, &state).await); }

        Commands::CreateRole {name} => { parse_result(create_role(name, &state).await); }
        Commands::ListRoles => { parse_result(list_roles(&state).await); }
        Commands::DeleteRole {name} => { parse_result(delete_role(name, &state).await); }
        Commands::AssignRole {role_name, user_id} => { parse_result(assign_role(role_name, user_id, &state).await); }
        Commands::RevokeRole {role_name, user_id} => { parse_result(revoke_role(role_name, user_id, &state).await); }
    }
}
