use crate::state::AppState;
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    path::Path,
};
use tracing::{error, info};

/* ------------------------------------------------ Constants ------------------------------------------------------------ */

pub const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f"; // Even though postgres has 6 digits by default, 3 are enough for the expected precision

/* ------------------------------------------------ Config ------------------------------------------------------------ */

// The directory where config files and other resources are stored
pub const SB_CONFIG_BASE_DIR: &str = "/etc/sensbee";
// When running in local mode we look in the project root dir for the config folder
pub const SB_CONFIG_BASE_DIR_LOCAL: &str = "config";

//
// Startup
//

pub fn from_config_dir(target_file_name: impl Into<String>) -> String {
    if !inside_compose_stack() {
        format!("{SB_CONFIG_BASE_DIR_LOCAL}/{}", target_file_name.into()).to_string()
    } else {
        format!("{SB_CONFIG_BASE_DIR}/{}", target_file_name.into()).to_string()
    }
}

//
// Runtime
//

// This flag is used to detect if we should assume a to be inside a compose stack environment.
pub const SB_FLAG_CONTAINERIZED: &str = "SB_CONTAINER";

pub fn inside_compose_stack() -> bool {
    match std::env::var(SB_FLAG_CONTAINERIZED.to_owned()) {
        Ok(_) => true,
        Err(_) => false,
    }
}

// Replaces the service name with "localhost" if the server is not running inside a compose stack network.
pub fn as_compose_service(service_name: impl Into<String>) -> String {
    if inside_compose_stack() {
        return service_name.into();
    } else {
        return "localhost".to_owned();
    }
}

//
// Local config file
//

pub const SB_CONFIG_FILE: &str = "config.yml";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    // General server config options
    pub server: Option<ServerServerConfig>,
    // Authentication options
    pub auth: Option<AuthConfig>,
}

// Parse the general config file
pub fn parse_config() -> anyhow::Result<ServerConfig> {
    let maybe_cfg_file = File::open(from_config_dir(SB_CONFIG_FILE));
    match maybe_cfg_file {
        Ok(cfg_file) => {
            let parsed_cfg: ServerConfig = serde_yml::from_reader(cfg_file).unwrap();
            info!("✅ Config parsed\n{:?}", parsed_cfg);
            return Ok(parsed_cfg);
        }
        Err(err) => {
            match err.kind() {
                std::io::ErrorKind::NotFound => {
                    // No override was provided and currently we dont use a default config file
                    Ok(ServerConfig {
                        server: None,
                        auth: None,
                    })
                }
                _ => Err(err.into()),
            }
        }
    }
}

// The database connection string to use
pub fn database_url() -> String {
    //
    dotenv().ok();
    // TODO add default values?
    format!(
        "postgres://{}:{}@{}:5432/{}",
        std::env::var("PSQL_USER").expect("PSQL_USER must be set"),
        std::env::var("PSQL_PASSWORD").expect("PSQL_PASSWORD must be set"),
        as_compose_service("postgres"),
        std::env::var("PSQL_DATABASE").expect("PSQL_DATABASE must be set")
    )
    .to_owned()
}

/* ------------------------------------------------ Server Options ------------------------------------------------------------ */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerServerConfig {
    // host and port to be used for the server
    host: Option<String>,
    port: Option<u16>,

    // host to be used for external callbacks
    external_host: Option<String>,

    // host where we should redirect to after succesfull OIDC auth
    external_sbmi_host: Option<String>,

    //
    run_mode: Option<String>,

    //
    ingest_max_size_kb: Option<usize>,
}

const CFG_SERVER_DEFAULT_HOST: &str = "localhost";
pub fn get_host(cfg: &ServerConfig) -> String {
    match &cfg.server {
        Some(srv_cfg) => match &srv_cfg.host {
            Some(h) => h,
            None => CFG_SERVER_DEFAULT_HOST,
        },
        None => CFG_SERVER_DEFAULT_HOST,
    }
    .to_string()
}

const CFG_SERVER_DEFAULT_PORT: u16 = 8080;
pub fn get_port(cfg: &ServerConfig) -> u16 {
    match &cfg.server {
        Some(srv_cfg) => match &srv_cfg.port {
            Some(h) => *h,
            None => CFG_SERVER_DEFAULT_PORT,
        },
        None => CFG_SERVER_DEFAULT_PORT,
    }
}

const CFG_SERVER_EXTERNAL_HOST: &str = "http://localhost:8080";
pub fn get_external_host(cfg: &ServerConfig) -> String {
    match &cfg.server {
        Some(srv_cfg) => match &srv_cfg.external_host {
            Some(v) => v,
            None => CFG_SERVER_EXTERNAL_HOST,
        },
        None => CFG_SERVER_EXTERNAL_HOST,
    }
    .to_string()
}

const CFG_SERVER_EXTERNAL_SBMI_HOST: &str = "http://localhost:8082";
pub fn get_external_sbmi_host(cfg: &ServerConfig) -> String {
    match &cfg.server {
        Some(srv_cfg) => match &srv_cfg.external_sbmi_host {
            Some(v) => v,
            None => CFG_SERVER_EXTERNAL_SBMI_HOST,
        },
        None => CFG_SERVER_EXTERNAL_SBMI_HOST,
    }
    .to_string()
}

// Check if we are explicitly not in development mode
pub fn is_prod_mode(cfg: &ServerConfig) -> bool {
    match &cfg.server {
        Some(server_cfg) => match &server_cfg.run_mode {
            Some(rm) => !(rm == "dev"),
            None => true,
        },
        None => true,
    }
}

// Same as the actix default value
const CFG_SERVER_DEFAULT_INGEST_MAX_SIZE_KB: usize = 256;
pub fn get_ingest_max_size_kb(cfg: &ServerConfig) -> usize {
    match &cfg.server {
        Some(srv_cfg) => match &srv_cfg.ingest_max_size_kb {
            Some(h) => *h,
            None => CFG_SERVER_DEFAULT_INGEST_MAX_SIZE_KB,
        },
        None => CFG_SERVER_DEFAULT_INGEST_MAX_SIZE_KB,
    }
}

/* ------------------------------------------------ Auth Options ------------------------------------------------------------ */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuthConfig {
    // Options for JWT
    pub jwt: Option<JWTParseConfig>,
    // List of OIDC configurations
    pub oidc_clients: Vec<OidcClient>,
    // Should new users be set as verified upon Authentication
    pub default_verified: Option<bool>,
    // Email of the root user
    pub root_user_email: Option<String>,
}

// Returns the name of the root user config value if it is set.
// This is intended to be used to assign the root role to an authenticated user upon successfull authentication
pub fn root_user_email(state: &AppState) -> Option<String> {
    match &state.cfg.auth {
        Some(auth_cfg) => match &auth_cfg.root_user_email {
            Some(v) => Some(v.to_string()),
            None => None,
        },
        None => None,
    }
}

const CFG_AUTH_DEFAULT_VERIFIED: bool = false;
// Used to govern wether a new User Authenticated via external IDP should be directly set as verified or not.
pub fn new_users_default_verified(state: &AppState) -> bool {
    match &state.cfg.auth {
        Some(auth_cfg) => match &auth_cfg.default_verified {
            Some(v) => *v,
            None => CFG_AUTH_DEFAULT_VERIFIED,
        },
        None => CFG_AUTH_DEFAULT_VERIFIED,
    }
}

/* ------------------------------------------------ Auth - OIDC ------------------------------------------------------------ */

// Representation of a single OIDC client
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OidcClient {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub issuer_url: String,
}

/* ------------------------------------------------ Auth - JWT ------------------------------------------------------------ */

// Config file options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JWTParseConfig {
    // Minutes until the generated key expires
    pub max_age: Option<i64>,
}

pub const CFG_AUTH_JWT_MAX_AGE_DEFAULT: i64 = 43200;
pub fn get_jwt_max_age(cfg: &ServerConfig) -> i64 {
    match &cfg.auth {
        Some(auth_cfg) => match &auth_cfg.jwt {
            Some(jwt_cfg) => match &jwt_cfg.max_age {
                Some(v) => *v,
                None => CFG_AUTH_JWT_MAX_AGE_DEFAULT,
            },
            None => CFG_AUTH_JWT_MAX_AGE_DEFAULT,
        },
        None => CFG_AUTH_JWT_MAX_AGE_DEFAULT,
    }
}

// Key-pair files that are used to override the default keys
pub const SB_CONFIG_FILE_JWT_PRIVATE_KEY: &str = "jwt/key.pem";
pub const SB_CONFIG_FILE_JWT_PUBLIC_KEY: &str = "jwt/key.pub.pem";

// Runtime config struct
#[derive(Debug, Clone)]
pub struct JWTConfig {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
    pub max_age: i64,
}

impl JWTConfig {
    // Requires that the server config has been loaded
    pub fn init(cfg: &ServerConfig) -> JWTConfig {
        // Default values
        let mut uses_default_keys = true;
        let mut jwt_private_key = DEF_JWT_KEY.as_bytes().to_owned();
        let mut jwt_public_key = DEF_JWT_PUBKEY.as_bytes().to_owned();
        let jwt_max_age = get_jwt_max_age(cfg);

        // Check if jwt files are present in the config
        let pkey_path = from_config_dir(SB_CONFIG_FILE_JWT_PRIVATE_KEY);
        if Path::new(&pkey_path).exists() {
            // Load Private key
            let fc = fs::read(&pkey_path);
            if fc.is_err() {
                error!("We failed to read {pkey_path} due to {}", fc.err().unwrap());
                std::process::exit(-1);
            }
            jwt_private_key = fc.unwrap();

            let key_path = from_config_dir(SB_CONFIG_FILE_JWT_PUBLIC_KEY);
            if !Path::new(&key_path).exists() {
                error!("We detected a secret for {SB_CONFIG_FILE_JWT_PRIVATE_KEY} but we are missing {SB_CONFIG_FILE_JWT_PUBLIC_KEY}");
                std::process::exit(-1);
            }

            // Load Public key
            let fc = fs::read(&key_path);
            if fc.is_err() {
                error!("We failed to read {key_path} due to {}", fc.err().unwrap());
                std::process::exit(-1);
            }
            jwt_public_key = fc.unwrap();

            info!("Using custom JWT keys");

            uses_default_keys = false;
        }

        // If we are in prod mode and still use the default keys we should not :)
        if is_prod_mode(cfg) && uses_default_keys {
            if !inside_compose_stack() {
                info!("Using default JWT keys");
            } else {
                #[cfg(not(test))]
                {
                    // Only tests are ok to use default keys in prod_mode!
                    error!("Lets not use default keys when running in prod!");
                    std::process::exit(-1);
                }
            }
        }

        JWTConfig {
            private_key: jwt_private_key.to_vec(),
            public_key: jwt_public_key,
            max_age: jwt_max_age,
        }
    }
}

// We have these keys in the git history anyway, might as well use them for default auth when running in dev mode
// NOTE we could also just generate them if none are set during server start?
const DEF_JWT_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQD1Xz/+kW5WMc18
hObk9BXE526nkiophuQaJrUXRFmYcJxkG1zTofSsXGLUgXU2Q1JEanA6eoQShIMc
N1yopAj6AL+rrufYgLsbTfpFTNBHJV0No96R2ARsRN2SSEAomZQ6+GOk8GSUFf9S
HQgm4ZOgJQS6WNuSm2Y38UlxdFlTZM8UnWvrCiCdaz4BAFMOiVFvVYFcpijV9Xn1
4ab1M59QDGX44RIzMCqZOX/bCZ6JO4UWhE2f3veFqKDXfcm/ZviWnrGZXHweuNl4
m3RcIPNsi2jVs+JPuGuiS1ihNKpvyaWF8VOFQ0NTZoQxKgnKArIN0p7089Wbl2tG
xW5ETXJPAgMBAAECggEAX66o8cgcUphRHQFmWFmmM4Zb7BRfRJpJULlPXKdWmM8w
7QiOhihdlOjh0SQ6ZNKTvgITiXpFDkOGLPaZt1lf9r1cAMLPvelVDSy3L1bo0RWM
18jS/eRStTWVgXmTK0HYP7akhKkJT4XUblzL1P2Z55UH5vLHjL/0eFQq44cPXijp
76nGopKwCi8myNkFV5LHU2O6d18wa86/Si3CupRazj8K6I4y7FjVhIvtheXVUNwy
GBN9C15WwW1B7wT159TqGZfQAaNwAQ9K8HG683sXw7traPOJtOrKbt7KjAis99Z7
VwS3neY5vKAwCNy8JNTkL7jwGM1/n34rOmDf5h/AQQKBgQD7Hzg2lkp6vFO0ssOQ
jDYZ/nSZCLl7YrKdDdzmywDzbsd7ECsLTTtlEC9J7uTuwYKggcuLn9VYs1S3nywK
h9zB06yE/ZJGaCt+bRu5dORSPkQO5YwygiQa6pbnat0FsZeb+MQw2wxQuEtCAZ8e
PJ7Q+8Sx6uKRjrjehI1oZYh5hwKBgQD6I2/2vW9xqSn1VM+Cl+GDG/tqHrJMnSQ4
IkSmwfzs2HnBwyfJpcSrmNACiS39kSIfzM1hzmX/eFU9XKH+4OH8RqIZwvuZKLcl
wYZtemOqDDp1LmR8Ec/ItQxMj5+jIpLP1ldr9vfXxXCjgKJj8Y1M2YGGxCGcEldA
BXQQN85S+QKBgDKyr9edafXzdXbCrGbPV9DRpVqL/15go6y/cryJeWysDcvTjM0g
T4Bszw8/Eqr9GFEtQxmyMBFTSyQzF2Ic4b2j7W1///sE5tsETheX+Mx906GqSC6e
RYnFmKep1Gtk2jXb+EfgwVC+lDsENsqU9H8+hQxiXlGCneIWorHT5cSbAoGAbU3a
+TkiX8qKGThsTSbNVpt9q4uqYiwwzY677RrLyTt3SSJcWpNjc5CdJN6JCErJXJgE
D38/tvkAVoYKC/R9C95Zq2Q2yWCvV4JPmbtnncEmMlqJcmXeJFho/XOtUH4lJUkG
fpk3CESwyeHGFGJTWoeZQCiofyjMk59Obl/UexECgYEAjtWdOYstsx00hcEcwKMQ
w+9Qn5CpR1jaVBYOaskXQpP1jbTug14dh8cnnIs2oa9LEKsbILXXKouURA8A5jpp
PdsJs2ZT9QlLIXxVlsZTpYdmA/yrOHOfkcMVNKrkGr59Jn/q3H4x9g2Vm6hE3/x+
71LJgYYHn2xMIJhYXYS7oFM=
-----END PRIVATE KEY-----";
const DEF_JWT_PUBKEY: &str = "-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA9V8//pFuVjHNfITm5PQV
xOdup5IqKYbkGia1F0RZmHCcZBtc06H0rFxi1IF1NkNSRGpwOnqEEoSDHDdcqKQI
+gC/q67n2IC7G036RUzQRyVdDaPekdgEbETdkkhAKJmUOvhjpPBklBX/Uh0IJuGT
oCUEuljbkptmN/FJcXRZU2TPFJ1r6wognWs+AQBTDolRb1WBXKYo1fV59eGm9TOf
UAxl+OESMzAqmTl/2wmeiTuFFoRNn973haig133Jv2b4lp6xmVx8HrjZeJt0XCDz
bIto1bPiT7hroktYoTSqb8mlhfFThUNDU2aEMSoJygKyDdKe9PPVm5drRsVuRE1y
TwIDAQAB
-----END PUBLIC KEY-----";
