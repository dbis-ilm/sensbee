use crate::authentication::openid::{init_oauth, OAuthState};
use crate::database::models::events::EventEngineState;
use crate::database::models::role::{ROLE_SYSTEM_ADMIN, ROLE_SYSTEM_ROOT};
use crate::database::role_db::assign_role;
use crate::database::user_db::{
    get_user_by_email, is_admin_user, is_root_user, register_user, verify_user,
};
use crate::features::cache;
use crate::features::cache::CachedData;
#[cfg(feature = "cache_sync")]
use crate::features::cache_sync::CacheSyncData;
use crate::features::config::{
    is_prod_mode, parse_config, root_user_email, JWTConfig, ServerConfig,
};
use crate::features::event_generation::init_event_service;
use crate::features::sensor_data_transform::{start_websocket_task, TransformService};
use crate::handler::data_ingest::ingest::{IngestStats, RuntimeIngestStats};
use crate::handler::data_ingest::mqtt::{mqtt_service_init, MQTT};
use crate::handler::models::requests::RegisterUserRequest;
use std::sync::Arc;
use tracing::{error, warn};

pub type AppState = Arc<SharedState>;

/// Application state shared across the system.
/// NOTE each test needs to be isolated from all other tests running in parallel.
#[derive(Clone)]
pub struct SharedState {
    // true if this instance has been started in prod
    pub is_prod: bool,

    // DB connection related member
    pub db: sqlx::PgPool,
    pub cache: Arc<CachedData>,
    #[cfg(feature = "cache_sync")]
    pub sync: Arc<CacheSyncData>,

    // Other services that need to be initialized when the app state is created.
    // Because they also depend on the database connection contained in this struct
    pub data_transform: Arc<TransformService>,
    pub mqtt_listener: Option<Arc<MQTT>>,

    // Logging & Event Service Channel
    pub events: Option<Arc<EventEngineState>>,

    // Stats collected during runtime to help with debugging and verification
    pub rt_stats: RuntimeIngestStats,

    // Server config
    pub cfg: Arc<ServerConfig>,
    // This config value is the same for all instances so can be safely copied at all times
    pub jwt: Arc<JWTConfig>,
    // OAuth state
    pub oauth: Arc<OAuthState>,
}

impl SharedState {
    fn new(pool: sqlx::PgPool) -> AppState {
        // Due to the fact that some inits require the AppState before it is fully initialized we create one
        // where these services are none and set afterwards
        // so during init those are not available!

        let cfg = parse_config().unwrap();

        // Initialize JWT config
        let jwt = JWTConfig::init(&cfg);

        // Start the caches
        let cache = Arc::new(cache::new_cache());

        //
        let mut state = SharedState {
            is_prod: is_prod_mode(&cfg),
            db: pool.clone(),
            cache,
            #[cfg(feature = "cache_sync")]
            sync: CacheSyncData::new(cache.clone(), pool.clone()),
            data_transform: Arc::new(start_websocket_task(pool.clone())),
            mqtt_listener: None,
            events: None,
            rt_stats: IngestStats::new(),
            jwt: Arc::new(jwt),
            oauth: Arc::new(init_oauth(&cfg)),
            cfg: Arc::new(cfg),
        };

        //
        state.events = Some(Arc::new(init_event_service(Arc::new(state.clone()))));

        // Now that we have an AppState struct and the event handling we can init the mqtt service which depends on the other things existing
        state.mqtt_listener = Some(Arc::new(mqtt_service_init(Arc::new(state.clone()))));

        // the now correctly initilaized AppState
        Arc::new(state)
    }
}

///
pub fn init_app_state(pool: sqlx::PgPool) -> AppState {
    let _ = env_logger::try_init_from_env(env_logger::Env::new().default_filter_or("info"));

    SharedState::new(pool)
}

/// When we are in a development mode and and have a root user set, then we create it with correct roles
pub async fn development_setup(state: &AppState) {
    // When in development mode, we check whether we need to setup a root user
    if !is_prod_mode(&state.cfg) {
        let root_email = match root_user_email(state) {
            Some(v) => v,
            None => {
                warn!("No root_user_email set");
                return;
            }
        };

        // make sure that the root user exists
        let root_user = match get_user_by_email(&root_email.clone(), state).await {
            Ok(v) => match v {
                Some(u) => u,
                None => {
                    // Need to create the new root user
                    match register_user(
                        RegisterUserRequest {
                            name: "Development Root".to_owned(),
                            email: root_email.clone(),
                        },
                        state,
                    )
                    .await
                    {
                        Ok(u) => u,
                        Err(err) => {
                            error!("get_user_by_email failed with {}", err);
                            return;
                        }
                    }
                }
            },
            Err(err) => {
                error!("get_user_by_email failed with {}", err);
                return;
            }
        };

        // Verify
        if !root_user.verified {
            let _ = verify_user(root_user.id, state).await;
        }

        // Assign admin role if not already assigned
        if !is_admin_user(root_user.id, state).await {
            let _ = match assign_role(root_user.id, ROLE_SYSTEM_ADMIN, true, state).await {
                Ok(_) => {}
                Err(err) => {
                    error!("assign_role admin role failed with {}", err);
                    return;
                }
            };
        }

        // Assign root role if not already assigned
        if !is_root_user(root_user.id, state).await {
            let _ = match assign_role(root_user.id, ROLE_SYSTEM_ROOT, true, state).await {
                Ok(_) => {}
                Err(err) => {
                    error!("assign_role root role failed with {}", err);
                    return;
                }
            };
        }

        log::info!("Root user was set up for '{}'", root_email);
    }
}
