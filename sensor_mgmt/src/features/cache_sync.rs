
use derive_more::derive::Display;
use serde::Deserialize;
use sqlx::postgres::PgListener;
use sqlx::PgPool;
use uuid::Uuid;
use std::collections::HashMap;
use std::sync::{Mutex, mpsc, Arc, Condvar, RwLock};
use std::sync::mpsc::Sender;
use serde_derive::Serialize;
use super::cache::{CachedData, MapType};

/*

Multi instance cache sync feature

In a scenario where multiple server instances are runnning behind a loabalancer, this feature
keeps the cache contents in sync with minimal latency.

The DB is used to broadcast cache changes. The current iteraion only snycs cache purge events 
which provides correctness after a short delay (local instance -> DB -> remote instances).

Architecture

When a cache purge is requested locally it must be send to the DB for distribution and also be 
executed lcoally.

When the cache gets created it also spawns a tokio runtime with two tasks

    Task: Listener worker [Wl]
        conncet to global sync channel via postegres listener
        loop
            Get global purge request
                Handle the request

    Task: Relay worker [Wr]
        Create channel that other functions can use to send cache purging requests
        loop
            Listen for cache purge requests
                Send them to the DB via pg_notify on the global channel

*/

#[derive(Debug,Display,Serialize,Deserialize,Clone)]
#[display("[{:?} -> {:?}]", target, data)]
pub struct SyncEvent {
    pub target: MapType,
    pub data: String,

    #[cfg(test)]
    pub id: String,
}

pub type EventHandlerCallback = fn(SyncEvent, Arc<CachedData>);

pub struct CacheSyncData {

    // channel to send SyncEvent job to the background thread
    // NOTE the order is important here. The tx must be dropped before the runtime
    // so that the futures all yield as the channel is closed
    // only then can the runtime be dropped. Otherwise we have a deadlock
    tx: Sender<SyncEvent>,

    // async runtime that executes the send/recieve threads
    rt: Arc<tokio::runtime::Runtime>,

    // map to register callbacks for events
    event_handler_registry: Arc<RwLock<HashMap<MapType, EventHandlerCallback>>>,

    #[cfg(test)]
    tasks: Arc<Mutex<HashMap<String, Arc<(Mutex<bool>, Condvar)>>>>,
}

impl CacheSyncData {

    pub fn new(cache: Arc<CachedData>, pool: PgPool) -> CacheSyncData {

        let (tx, rx) = mpsc::channel::<SyncEvent>();
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _enter = rt.enter();
        
        let reg: Arc<RwLock<HashMap<MapType, EventHandlerCallback>>> = Arc::new(RwLock::new(HashMap::default()));

        let tasks: Arc<Mutex<HashMap<String, Arc<(Mutex<bool>, Condvar)>>>> = Arc::new(Mutex::new(HashMap::default()));

        // 
        {
            let mut wguard = reg.write().unwrap();

            wguard.insert(MapType::Roles,handle_synced_role_purge_request);
            wguard.insert(MapType::Users, handle_synced_user_purge_request);
            wguard.insert(MapType::Sensors, handle_synced_sensor_purge_request);
            wguard.insert(MapType::ApiKeys, handle_synced_apikeys_purge_request);
        }
    
        #[cfg(test)]
        println!("[Wx] [{:?}] Starting the threads", std::time::SystemTime::now());

        let pg_channel_name = "cacheSync";
    
        // listener worker recieves sync events and handles them
        let arc_pool = pool.clone();
        let arc_reg = Arc::clone(&reg);
        let arc_tasks = Arc::clone(&tasks);
        rt.spawn(async move {
            // Setup to listen on cache_sync channel
            let mut listener = match PgListener::connect_with(&arc_pool).await {
                Err(e) => {
                    panic!("Failed to create PgListener: {}", e)
                }
                Ok(v)=>v
            };
            match listener.listen(pg_channel_name).await {
                Err(e) => {
                    panic!("Failed to listen on cache_sync channel: {}", e)
                }
                Ok(())=>()
            }
    
            #[cfg(test)]
            println!("[Wl] [{:?}] Running...", std::time::SystemTime::now());
    
            // Wait for postgres to send us events
            loop {
                match listener.recv().await {
                    Err(e)=>{
                        match e {
                            // When the connection is dropped we gracefully stop this thread
                            sqlx::Error::PoolClosed => {
    
                                #[cfg(test)]
                                println!("[Wl] [{:?}] Stopping with: PoolClosed", std::time::SystemTime::now());
    
                                return;
                            }
                            _ => {
                                panic!("[Wl] [{:?}] Rx error: {}", std::time::SystemTime::now(), e);
                            }
                        }
                    }
                    Ok(e)=>{
    
                        #[cfg(test)]
                        println!("[Wl] [{:?}] DB event rx: {:?}", std::time::SystemTime::now(), e);
    
                        let se: SyncEvent = serde_json::from_str(e.payload()).unwrap();

                        #[cfg(test)]
                        let se_cloned = se.clone();

                        {
                            let guard = arc_reg.read().unwrap();
                            if let Some(&cb) = guard.get(&se.target).clone() {
                                cb(se, Arc::clone(&cache));
                            } else {
                                #[cfg(test)]
                                println!("[Wl] [{:?}] no cb for {}", std::time::SystemTime::now(), se.target);
                            }

                            #[cfg(test)] {
                                let mut g = arc_tasks.lock().unwrap();
                                let r = g.remove(&se_cloned.id).unwrap();
                                println!("tasks remaining {}", g.len());
                                let (lock, cvar) = &*r;
                                let mut result = lock.lock().unwrap();
                                *result = true;
                                cvar.notify_all();
                            }
                        }                        
                    }
                }
            }
        });
    
        // Relay worker sends local sync events to postgres
        let arc_pool = pool.clone();
        rt.spawn(async move {
    
            #[cfg(test)]
            println!("[Wr] [{:?}] Running...", std::time::SystemTime::now());
    
            // Wait for local events and send them to postgres on the cache_sync channel
            for e in rx {
    
                #[cfg(test)]
                println!("[Wr] [{:?}] local event: {}", std::time::SystemTime::now(), e.to_string());
                        
                let data = serde_json::to_string(&e).unwrap();
                match sqlx::query("SELECT pg_notify($1, $2)")
                    .bind(pg_channel_name)
                    .bind(data)
                    .execute(&arc_pool)
                    .await
                {
                    Err(err) => match err {
                        // When the connection is dropped we gracefully stop this thread
                        sqlx::Error::PoolClosed => {
    
                            #[cfg(test)]
                            println!("[Wr] [{:?}] Stopping with: PoolClosed", std::time::SystemTime::now());
    
                            return;
                        }
                        _ => {
                            panic!("[Wr] [{:?}] Rx error: {}", std::time::SystemTime::now(), err);
                        }
                    },
                    Ok(_) => {
    
                        #[cfg(test)]
                        println!("[Wr] [{:?}] local event send: {}", std::time::SystemTime::now(), e.to_string());
    
                    },
                }
            } 
        });

        CacheSyncData{
            rt: Arc::new(rt),
            tx,
            event_handler_registry: reg,

            #[cfg(test)]
            tasks,
        }
    }

    fn register(&self, k: MapType, f: EventHandlerCallback) {

        #[cfg(test)]
        println!("Registering callback for {}", k);

        let mut wguard = self.event_handler_registry.write().unwrap();
        wguard.insert(k, f);
    }

    // Sends a SyncEvent to the database which is then broadcast to all instances connected to the same DB
    // NOTE during testing this functions blocks until that task is done
    pub fn send(&self, target: MapType, message: String) {

        #[cfg(not(test))]
        self.tx.send(SyncEvent { 
            target: target.clone(), 
            data: message.clone(),
        }).expect("Failed to send SyncEvent");
        
        #[cfg(test)] {
            // Prepare map entry to make this a blocking call
            let id = &Uuid::new_v4().to_string()[..8];
            let p = Arc::new((Mutex::new(false), Condvar::new()));
            {
                let mut g = self.tasks.lock().unwrap();
                g.insert(id.to_string(), Arc::clone(&p));
            }

            println!("sending {} -> {} {}", target, message, id.to_string());
        
            // Send the event / task
            self.tx.send(SyncEvent { 
                target: target.clone(), 
                data: message.clone(),
                id: id.to_string(),
            }).expect("Failed to send SyncEvent");
        
            // wait until the task has been processed
            let (lock, cvar) = &*p;
            let mut started = lock.lock().unwrap();
            while !*started {
                started = cvar.wait(started).unwrap();
            }

            println!("finalized {}", id.to_string());
        }
    }
}


/* ------------------------------------------------ Purge Event handler ------------------------------------------------------------ */

pub fn handle_synced_role_purge_request(se: SyncEvent, cache: Arc<CachedData>) {

    #[cfg(test)]
    println!("purging roles with {} ", se);

    match se.target {
        MapType::Roles => {
            if se.data.len() == 0 {
                cache.roles.write().unwrap().clear();
            } else {
                cache.roles.write().unwrap().remove(&se.data);
            }
        }
        _ => panic!("synced_role_purge received unexpected target type {}", se),
    }
}

pub fn handle_synced_user_purge_request(se: SyncEvent, cache: Arc<CachedData>) {

    #[cfg(test)]
    println!("purging users with {} ", se);

    match se.target {
        MapType::Users => {
            if se.data.len() == 0 {
                cache.users.write().unwrap().clear();
            } else {
                cache.users.write().unwrap().remove(&Uuid::parse_str(&se.data).unwrap());
            }
        }
        _ => panic!("synced_user_purge received unexpected target type {}", se),
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SensorPurgeEvent {
    pub is_sensor_id: bool,
    pub key: Uuid,
}

pub fn handle_synced_sensor_purge_request(se: SyncEvent, cache: Arc<CachedData>) {

    #[cfg(test)]
    println!("purging sensors with {} ", se);

    match se.target {
        MapType::Sensors => {
            if se.data.len() == 0 {
                cache.sensors.write().unwrap().clear();
            } else {
                // Handle uuid types
                let d: SensorPurgeEvent = serde_json::from_str(&se.data).unwrap();
                if d.is_sensor_id {
                    //  uuid -> sensor
                    cache.sensors.write().unwrap().remove(&d.key);
                } else {
                    //  uuid -> user_id
                    cache.sensors.write().unwrap().retain(|_, s| s.owner.is_none() || s.owner.unwrap() != d.key);
                }
            }
        }
        _ => panic!("synced_sensor_purge received unexpected target type {}", se),
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ApiKeyPurgeEvent {
    pub is_apikey: bool,
    pub is_sensor: bool,
    pub is_user: bool,
    pub key: Uuid,
}

pub fn handle_synced_apikeys_purge_request(se: SyncEvent, cache: Arc<CachedData>) {

    #[cfg(test)]
    println!("purging apikeys with {} ", se);

    match se.target {
        MapType::ApiKeys => {
            if se.data.len() == 0 {
                // Empty payload indicates that we want to clear all values of the cache
                cache.api_keys.write().unwrap().clear();
            } else {
                // Handle uuid types
                let d: ApiKeyPurgeEvent = serde_json::from_str(&se.data).unwrap();
                if d.is_apikey {
                    //  uuid -> api_key
                    cache.api_keys.write().unwrap().remove(&d.key);
                }
                if d.is_sensor {
                    //  uuid -> sensor
                    cache.api_keys.write().unwrap().retain(|_, v| v.sensor_id != d.key);
                }
                if d.is_user {
                    //  uuid -> user_id
                    cache.api_keys.write().unwrap().retain(|_, v| v.user_id != d.key);
                }
            }
        }
        _ => panic!("synced_apikeys_purge received unexpected target type {}", se),
    }
}

/* ------------------------------------------------ Tests ------------------------------------------------------------ */


#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::create_test_app;
    use sqlx::PgPool;

    fn cb_test(e: SyncEvent, _cache: Arc<CachedData>) {        
        assert!(e.data == "Test message");
    }
    
    #[sqlx::test(migrations = "../migrations", fixtures("../handler/fixtures/users.sql", "../handler/fixtures/roles.sql"))]
    pub async fn test_send_message(pool: PgPool) {
        let (_app, state) = create_test_app(pool.clone()).await;

        state.sync.register(MapType::Roles, cb_test);

        state.sync.send(MapType::Roles,"Test message".to_string());
    }
}

