use colored::Colorize;

pub fn print_welcome_message() {
    let msg = r#"
Commands can span multiple lines and are submitted with CTRL+X.
Enter 'help' for more on available commands."#;
    println!("{}", " ____                 ____            \n/ ___|  ___ _ __  ___| __ )  ___  ___ \n\\___ \\ / _ \\ '_ \\/ __|  _ \\ / _ \\/ _ \\\n ___) |  __/ | | \\__ \\ |_) |  __/  __/\n|____/ \\___|_| |_|___/____/ \\___|\\___|".blue().bold());
    println!();

    println!("{}", "SensBee command line interface v. 0.3".blue().bold());
    println!("{}", msg)
}

pub fn print_help_message() {
    let msg = r#"Available commands:
help   - print this help message
user   - manage users: create, list, edit, delete
sensor - manage sensors: list, create, edit, delete 
role   - manage roles: list, create, delete
ingest - ingest data to sensor tables from different sources
data   - show sensor data
Use <command> help for further details.
    "#;
    println!("{}", "SensBee command line interface v. 0.3".blue().bold());
    println!("{}", msg)
}

pub fn print_sensor_help_message() {
    let msg = r#"Available subcommands for sensor:
list   - print a list of all registered sensors
    sensor list
info   - print information about the sensor given by the id
    sensor info <sensor_id>
create - create a new sensor
    sensor create {
        "columns":[
            {"name":"count","val_type":"INT","val_unit":"number","val_ingest":"INCREMENTAL"},
            {"name":"temperature","val_type":"FLOAT","val_unit":"celsius","val_ingest":"LITERAL"}
        ],
        "description":"This is my first sensor.",
        "name":"MySensor",
        "position":[50.68322,10.91858]}
delete - delete the sensor given by the id
    sensor delete <sensor_id>
delete_key - delete the API key for the sensor given by the id
    sensor delete_key <sensor_id> <key_id>
create_write_key - create a new API key for writing data to the sensor
    sensor create_write_key <sensor_id> <key_name>
create_read_key - create a new API key for reading data from the sensor
    sensor create_read_key <sensor_id> <key_name>
help   - print this help message"#;
    println!("{}", msg)
}

pub fn print_ingest_help_message() {
 let msg = r#"Available subcommands for ingest:
file   - ingest data from a file to the sensor identified by the id
    ingest <sensor_id> file 'data.csv'
json   - ingest data in JSON format to the sensor identified by the id
    ingest <sensor_id> json [{ "timestamp": "2025-08-01T19:59:27.598455177", 
        "count": 42, "temperature": 27.5 }, { ... }]
help   - print this help message"#;
    println!("{}", msg)
}

pub fn print_data_help_message() {
 let msg = r#"Available subcommands for data:
show   - show data of the sensor identified by the id
    data show <sensor_id> 
help   - print this help message"#;
    println!("{}", msg)
}


pub fn print_user_help_message() {
 let msg = r#"Available subcommands for user:
me     - print information about the current user
list   - list all registered users
    user list
info   - print information about the user with the given id
    user info <user_id>
create - create a new user 
help   - print this help message"#;
    println!("{}", msg)
}

pub fn print_role_help_message() {
 let msg = r#"Available subcommands for role:
list   - list all roles
    role list
create - create a new role 
help   - print this help message"#;
    println!("{}", msg)
}

#[macro_export]
macro_rules! not_implemented {
    () => {
        return Err(anyhow!("not implemented"))
    };
    ($msg:expr) => {
        return Err(anyhow!("not implemented: {}", $msg))
    }
}