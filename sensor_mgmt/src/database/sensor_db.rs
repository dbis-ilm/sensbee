use anyhow::Result;
use serde_json::{Map, Value};
use sqlx::{query_builder::QueryBuilder, Execute, PgConnection, Postgres, Row, Transaction};
use crate::handler::models::responses::GenericUuidResponse;
use crate::{database::models::sensor_perm::SensorPermission, state::AppState};
use crate::database::models::api_key::ApiKey;
use crate::database::models::sensor::{ColumnType, FullSensorInfo, SensorColumn, ShortSensorInfo};
use crate::database::models::db_structs::{DBOperation, DBOrdering};
use crate::database::models::role::ROLE_SYSTEM_GUEST;
use crate::database::models::user::UserInfo;
use crate::features::cache;
use crate::features::sensor_data_storage::{register_sensor_data_storage, unregister_sensor_data_storage};
use crate::handler::models::requests::{DataLoadRequestParams, CreateApiKeyRequest, CreateSensorRequest, EditSensorRequest, SensorPermissionRequest};
use crate::features::user_sens_perm::{UserSensorPerm, UserSensorPermissions};

/// Return a list of all registered sensors.
pub async fn get_sensor_overview(state: &AppState) -> Result<Vec<ShortSensorInfo>> {
    //TODO: Can we cache this as well?
    let query_result = sqlx::query_as!(ShortSensorInfo, r#"
        SELECT id, name FROM sensor s"#)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(); 
    Ok(query_result)
}

/// Retrieve the complete information about a sensor from the database and return it
/// in the form of a SensorInfoResponse object. Should only be called from the cache and not directly.
pub async fn get_full_sensor_info(sensor_id: uuid::Uuid, conn: &mut PgConnection) -> Result<FullSensorInfo> {
    // Retrieve the data from the sensor and sensor_schema tables
    let query_result = sqlx::query(r#"
        SELECT id, s.name AS name, tbl_name, longitude, latitude, description, owner, storage_type, storage_params, col_name, col_type, col_unit
        FROM sensor s JOIN sensor_schema c ON s.id = c.sensor_id
        WHERE s.id = $1"#)
        .bind(sensor_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default(); 
    
    if query_result.is_empty() {
        anyhow::bail!("row not found");
    }

    let name = query_result[0].get("name");   
    let tbl_name = query_result[0].get("tbl_name");
    let longitude: Option<f64> = query_result[0].get("longitude");
    let latitude: Option<f64> = query_result[0].get("latitude");
    let description = query_result[0].get("description");
    let owner: Option<uuid::Uuid> = query_result[0].get("owner");
    let raw_storage_type: String = query_result[0].get("storage_type");
    let raw_storage_params: String = query_result[0].get("storage_params");
    
    let storage_type = serde_json::from_str(&raw_storage_type)?;
    let storage_params = match serde_json::from_str::<Value>(&raw_storage_params) {
        Ok(Value::Object(map)) => Some(map), // If it's a JSON object, return the map
        _ => None,
    };

    let sensor_position: Option<(f64, f64)> = if longitude.is_some() && latitude.is_some() {
        Some((latitude.unwrap(), longitude.unwrap()))
    } else { 
        None 
    };

    // we have to build a vector with all column information from the join result
    let mut columns: Vec<SensorColumn> = Vec::with_capacity(query_result.len());
    for row in query_result {
        let ctype: i32 = row.get("col_type");
        columns.push(SensorColumn {
                name: row.get("col_name"),
                val_type: ColumnType::from_integer(ctype),
                val_unit: row.get("col_unit"),
        });
    }

    // Get sensor permissions
    
    let perm_res = sqlx::query_as!(SensorPermission, "SELECT * FROM sensor_permissions WHERE sensor_id=$1", sensor_id)
        .fetch_all(&mut *conn)
        .await;

    if let Err(err) = perm_res {
        println!("Couldn't fetch permissions for sensor {}!", sensor_id);
        anyhow::bail!(err)
    }

    // construct and return the SensorInfoResponse object
    Ok(FullSensorInfo {
        id: sensor_id, 
        name,
        description,
        position: sensor_position,
        tbl_name,
        owner,
        columns,
        permissions: perm_res?,
        storage_type,
        storage_params
    })
}

/// Delete all information about the sensor and its data table from the database. The sensor
/// is identified by a UUID.
pub async fn delete_sensor(sensor_id: uuid::Uuid, state: &AppState) -> Result<()> {
    let sensor = cache::request_sensor(sensor_id, &state).await.ok_or_else(|| anyhow::anyhow!("No sensor with id {}!", sensor_id))?;

    let mut tx = state.db.begin().await?;
 
    // construct the DROP TABLE statement for the data table
    let mut drop_stmt = String::new();
    drop_stmt.push_str(format!("DROP TABLE {}", sensor.tbl_name).as_str());
    
    // ... and execute it
    let query_result = sqlx::query(&drop_stmt)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // next, delete the schema information from the sensor_schema table
    let query_result = sqlx::query!(r#"DELETE FROM sensor_schema WHERE sensor_id = $1"#, sensor_id.clone())
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // next, remove the sensor data storage
    let res = unregister_sensor_data_storage(sensor.storage_type, sensor.tbl_name.clone(), tx.as_mut()).await;

    if let Err(err) = res {
        let _ = tx.rollback().await;
        anyhow::bail!(err);
    }

    // next, delete the sensor permissions
    let query_result = sqlx::query!(r#"DELETE FROM sensor_permissions WHERE sensor_id = $1"#, sensor_id.clone())
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // next, delete the existing api_keys
    let query_result = sqlx::query!("DELETE FROM api_keys WHERE sensor_id = $1", sensor_id.clone())
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // and finally, from the sensor table
    let query_result = sqlx::query!(r#"DELETE FROM sensor WHERE id = $1"#, sensor_id.clone())
    .execute(&mut *tx)
    .await
    .map_err(|err: sqlx::Error| err.to_string());
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // now, we can commit the transaction
    let _ = tx.commit().await;

    cache::purge_sensor(sensor_id, state);
    cache::purge_api_keys_for_sensor(sensor_id, state);

    Ok(())
}

/// Edits information about an existing sensor.
/// Only attributes specified by the EditSensorRequest object may be modified.
pub async fn edit_sensor(sensor_id: uuid::Uuid, body: EditSensorRequest, state: &AppState) -> Result<()> {
    let sensor = cache::request_sensor(sensor_id, &state).await.ok_or_else(|| anyhow::anyhow!("No sensor with id {}!", sensor_id))?;
    
    let (lat, long) = match body.position {
        Some((lat, long)) => (Some(lat), Some(long)),
        None => (None, None),
    };

    let mut tx = state.db.begin().await?;

    let query_result =
        sqlx::query(r#"UPDATE sensor SET name=$1, description=$2, longitude=$3, latitude=$4, storage_type=$5, storage_params=$6 WHERE id=$7"#)
            .bind(body.name)
            .bind(body.description)
            .bind(long)
            .bind(lat)
            .bind(serde_json::to_string(&body.storage.variant)?)
            .bind(serde_json::to_string(&body.storage.params)?)
            .bind(sensor_id.clone())
            .execute(&mut *tx)
            .await
            .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        println!("UPDATE of sensor failed");
        anyhow::bail!(err)
    }

    // Edit data storage strategy (if changed)
    
    if sensor.storage_type != body.storage.variant || sensor.storage_params != body.storage.params {
        // Remove old strategy
        let res = unregister_sensor_data_storage(sensor.storage_type, sensor.tbl_name.clone(), tx.as_mut()).await;

        if let Err(err) = res {
            let _ = tx.rollback().await;
            println!("Couldn't remove old data storage strategy for sensor with id {}!", sensor_id);
            anyhow::bail!(err);
        }

        // Add new strategy
        let res = register_sensor_data_storage(body.storage.variant, sensor.tbl_name, body.storage.params, tx.as_mut()).await;

        if let Err(err) = res {
            let _ = tx.rollback().await;
            println!("Couldn't setup new data storage for sensor with id {}!", sensor_id);
            anyhow::bail!(err);
        }
    }

    // Edit permissions

    let perm_query = update_sensor_access(sensor_id, body.permissions, &state, &mut tx).await;

    if let Err(err) = perm_query {
        let _ = tx.rollback().await;
        println!("Couldn't set permissions for sensor with id {}!", sensor_id);
        anyhow::bail!(err);
    }

    let _ = tx.commit().await;

    cache::purge_sensor(sensor_id, state);

    Ok(())
}

/// Register a new sensor by creating the schema information and creating the data table.
/// All the required information are passed via a CreateSensorRequest object.
/// If a user is given, it becomes the owner of the sensor.
pub async fn create_sensor(body: CreateSensorRequest, user_id: Option<uuid::Uuid>, state: &AppState) -> Result<GenericUuidResponse> {
    // create a new UUID
    let sensor_id = uuid::Uuid::new_v4();

    // Reuse UUID as the table name with a prefix, but remove all dashes
    let table_name = format!("s_{}", sensor_id.to_string().replace("-", ""));

    let mut tx = state.db.begin().await?;

    let (lat, long) = match body.position {
        Some((lat, long)) => (Some(lat), Some(long)),
        None => (None, None),
    };

    let query_result =
        sqlx::query(r#"INSERT INTO sensor (id, name, description, longitude, latitude, tbl_name, owner, storage_type, storage_params) 
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#)
            .bind(sensor_id.clone())
            .bind(body.name.to_string())
            .bind(body.description)
            .bind(long)
            .bind(lat)
            .bind(table_name.clone())
            .bind(user_id.map(|u| u))
            .bind(serde_json::to_string(&body.storage.variant)?)
            .bind(serde_json::to_string(&body.storage.params)?)
            .execute(&mut *tx)
            .await
            .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        println!("INSERT INTO sensor failed");
        anyhow::bail!(err)
    }

    let mut create_stmt = String::new();
    create_stmt.push_str(format!("CREATE TABLE {} ( created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP", table_name).as_str());

    let columns = &body.columns;
    for col in columns.iter() {
        create_stmt.push_str(", ");
        let sql_type = match col.val_type {
            ColumnType::INT => "INT",
            ColumnType::FLOAT => "FLOAT",
            _ => "VARCHAR(50)"
        };
        create_stmt.push_str(format!("{} {}", &col.name, sql_type).as_str());
  
        let query_result = 
        sqlx::query(r#"
            INSERT INTO sensor_schema (sensor_id, col_name, col_type, col_unit) 
            VALUES ($1, $2, $3, $4)
            "#)
            .bind(sensor_id.clone())
            .bind(col.name.to_string())
            .bind(col.val_type as i32)
            .bind(col.val_unit.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|err: sqlx::Error| err.to_string());

        if let Err(err) = query_result {
            let _ = tx.rollback().await;
            println!("INSERT INTO sensor_schema failed");
            anyhow::bail!(err)
        }
    }

    create_stmt.push_str(")");
    // create TABLE
    let query_result = sqlx::query(&create_stmt)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        println!("CREATE TABLE sensor... failed");
        anyhow::bail!(err)
    }
    
    // Setup data storage strategy

    let data_storage_res = register_sensor_data_storage(body.storage.variant, table_name, body.storage.params, tx.as_mut()).await;

    if let Err(err) = data_storage_res {
        let _ = tx.rollback().await;
        println!("Couldn't setup data storage for sensor with id {}!", sensor_id);
        anyhow::bail!(err);
    }
    
    // Setup permissions

    let perm_query = update_sensor_access(sensor_id, body.permissions, &state, &mut tx).await;

    if let Err(err) = perm_query {
        let _ = tx.rollback().await;
        println!("Couldn't set permissions for sensor with id {}!", sensor_id);
        anyhow::bail!(err);
    }

    let _ = tx.commit().await;

    Ok(GenericUuidResponse{uuid: sensor_id.to_string()})
}

/* ------------------------------------------------ Data Management ------------------------------------------------------------ */

/// Insert the sensor data given by the JSON object to the given table.
async fn add_data_to_table(sensor: &FullSensorInfo, body: &Value, state: &AppState) -> Result<()> {
    // INSERT INTO sensor.tbl_name () VALUES ()
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO ");
    query_builder.push(sensor.tbl_name.clone() + " (");

    // Push all columns of the sensor to query
    let mut cols_sep = query_builder.separated(", ");
    
    for col in sensor.columns.iter() {
        cols_sep.push(&col.name);
    }

    cols_sep.push_unseparated(") VALUES (");

    let column_values = body.as_object().unwrap();

    // Only insert data for valid cols into the sensor, otherwise NULL

    let mut valid_cols = 0;
    let mut vals_sep = query_builder.separated(", ");
    
    for col in sensor.columns.iter() {
        let prov_col_val = column_values.get(&col.name);
        
        if prov_col_val.is_some() {
            valid_cols += 1;
            
            // Column is valid, parse provided value or NULl if invalid
            match &col.val_type {
                ColumnType::INT => { vals_sep.push_bind(prov_col_val.unwrap().as_i64());}
                ColumnType::FLOAT => { vals_sep.push_bind(prov_col_val.unwrap().as_f64()); }
                ColumnType::STRING => { vals_sep.push_bind(prov_col_val.unwrap().as_str());}
                _ => {}
            }
        } else {
            match &col.val_type {
                ColumnType::INT => { vals_sep.push_bind(None::<i64>);}
                ColumnType::FLOAT => { vals_sep.push_bind(None::<f64>); }
                ColumnType::STRING => { vals_sep.push_bind(None::<String>);}
                _ => {}
            }
        }
    }

    vals_sep.push_unseparated(") ");
    
    if valid_cols == 0 {
        anyhow::bail!("No valid columns to insert data into the sensor {}!", sensor.id);
    }
    
    // Execute query

    let query = query_builder.build();

    let mut tx = state.db.begin().await?;

    let res = query.execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());
    
        if let Err(err) = res {
            anyhow::bail!(err)
        }

    let _ = tx.commit().await;
    
    Ok(())
}

/// Add sensor measurement data given as JSON data to the sensor data table. 
/// The sensor is identified by the 'sensor_id' field, the columns and their values
/// are also given as fields the JSON object, e.g. "column1": value.
pub async fn add_sensor_data(sensor_id: uuid::Uuid, body: &Value, state: &AppState) -> Result<()> {
    let sensor = cache::request_sensor(sensor_id, &state).await;

    if sensor.is_none() {
        anyhow::bail!("Sensor with id {} not found!", sensor_id);
    }

    add_data_to_table(&sensor.unwrap(), body, state).await
}

/// Fetches data specified by the given predicates in SensorDataRequest.
pub async fn get_data(sensor_id: uuid::Uuid, request: DataLoadRequestParams, state: &AppState) -> Result<Value> {
    let sensor = cache::request_sensor(sensor_id, &state).await;

    if sensor.is_none() {
        anyhow::bail!("Sensor with id {} not found!", sensor_id);
    }

    let sensor = sensor.unwrap();

    // --- Create Select, From query ---
    
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT ");
    let mut separated = query_builder.separated(", ");

    separated.push("created_at");
    for column in sensor.columns.as_slice() {
        separated.push(column.name.clone());
    }
    separated.push_unseparated(" FROM ");
    query_builder.push(sensor.tbl_name.clone());

    // --- Handle predicates based on the request ---

    let mut predicates: Vec<String> = Vec::new();

    if request.from.is_some() {
        predicates.push(format!("created_at >= {}", request.from.unwrap().format(&format!("'{}'", crate::utils::TIMESTAMP_FORMAT).to_string())));
    }

    if request.to.is_some() {
        predicates.push(format!("created_at <= {}", request.to.unwrap().format(&format!("'{}'", crate::utils::TIMESTAMP_FORMAT).to_string())));
    }
    
    if !predicates.is_empty() {
        query_builder.push(" WHERE ");
        
        for (idx, predicate) in predicates.iter().enumerate() {
            if idx != 0 {
                query_builder.push(" AND ");
            }
            
            query_builder.push(predicate);
        }
    }

    // --- Handle ordering and limit ---
    
    if request.ordering.is_some() {
        match request.ordering.unwrap() {
            DBOrdering::ASC => query_builder.push(" ORDER BY created_at ASC"),
            DBOrdering::DESC => query_builder.push(" ORDER BY created_at DESC"),
            DBOrdering::DEFAULT => query_builder.push(""),
        };
    }
    
    if request.limit.is_some() {
        query_builder.push(format!(" LIMIT {}", request.limit.unwrap()));
    }
    
    // --- Build and execute query ---

    let query = query_builder.build();
    
    let sql = query.sql();
    log::debug!("query: {}", sql);

    let query_result = query.fetch_all(&state.db)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if query_result.is_err() {
        println!("{:?}", query_result.unwrap_err());
        anyhow::bail!("Couldn't fetch sensor data with the specified predicates!");
    }

    // --- Parse result ---
    
    let query_result = query_result.unwrap();

    let mut array = Vec::<serde_json::Value>::new();
    for row in query_result {
        let mut map = Map::new();
        let created_at: chrono::NaiveDateTime = row.get("created_at");
        map.insert("created_at".to_string(), serde_json::json!(created_at.to_string()));
        
        // Extracts values for each column, inserting None if columns contains NULL or value is not parsable
        
        for col in sensor.columns.as_slice() {
            let colname = col.name.to_lowercase();
            match &col.val_type {
                ColumnType::INT => {
                    match row.try_get::<Option<i32>, _>(colname.as_str()) {
                        Ok(val) => {
                            map.insert(colname.clone(), serde_json::json!(val));
                        },
                        Err(err) => {
                            eprintln!("Error retrieving INT value for column '{}': {}", colname, err);
                        }
                    }
                }
                
                ColumnType::FLOAT => {
                    match row.try_get::<Option<f64>, _>(colname.as_str()) {
                        Ok(val) => {
                            map.insert(colname.clone(), serde_json::json!(val));
                        },
                        Err(err) => {
                            eprintln!("Error retrieving FLOAT value for column '{}': {}", colname, err);
                        }
                    }
                }
                
                ColumnType::STRING => {
                    match row.try_get::<Option<String>, _>(colname.as_str()) {
                        Ok(val) => {
                            map.insert(colname.clone(), serde_json::json!(val));
                        },
                        Err(err) => {
                            eprintln!("Error retrieving STRING value for column '{}': {}", colname, err);
                        }
                    }
                }
                _ => {}
            }
        }

        array.push(serde_json::json!(map));
    }

    Ok(serde_json::json!(array))
}

/* ------------------------------------------------ Access Management ------------------------------------------------------------ */

/// Retrieves a list of permissions the user has for the specified sensor.
pub async fn get_user_sensor_permissions(user_id: Option<uuid::Uuid>, sensor: &FullSensorInfo, state: &AppState) -> UserSensorPermissions {
    let user = match user_id {
        Some(u) => {
            let user = cache::request_user(u, &state).await;

            if user.is_none() {
                println!("User not found with id {}!", user_id.unwrap());
                return UserSensorPermissions::new();
            }

            user
        },
        None => None
    };

    get_user_sensor_permissions_impl(user.as_ref(), sensor, state).await
}

/// Retrieves a list of permissions the user has for the specified sensor.
pub async fn get_user_sensor_permissions_impl(user: Option<&UserInfo>, sensor: &FullSensorInfo, state: &AppState) -> UserSensorPermissions {
    let mut permissions = UserSensorPermissions::new();

    // Owner has always full access
    if user.is_some() && sensor.is_owner(user.as_ref().map(|u| u.id).unwrap()) {
        permissions.add_all();
        
        return permissions;
    }

    // Get roles for the user, if a user is provided
    let mut user_roles = match user {
        Some(u) => u.roles.clone(),
        None => vec![]
    };

    // Add guest role for both, provided and anonymous users
    let guest_role = cache::request_role(ROLE_SYSTEM_GUEST, &state).await.unwrap();
    user_roles.push(guest_role.clone());

    for role in user_roles.iter() {
        // Admin has full permissions to sensor
        if role.is_admin() {
            permissions.add_all();

            return permissions;
        }

        // Iterate all sensor permissions and check matches

        for perm in sensor.permissions.iter() {
            if perm.role_id == role.id {
                if perm.allow_info {
                    permissions.add(UserSensorPerm::Info);
                }

                if perm.allow_read {
                    permissions.add(UserSensorPerm::Read);
                    
                    if perm.role_id != guest_role.id { // Guests can't create keys
                        permissions.add(UserSensorPerm::ApiKeyRead);
                    }
                }

                if perm.allow_write {
                    permissions.add(UserSensorPerm::Write);
                    
                    if perm.role_id != guest_role.id { // Guests can't create keys
                        permissions.add(UserSensorPerm::ApiKeyWrite);
                    }
                }
            }
        }
    }
    
    permissions
}

/// Updates access to the given sensor for a batched of defined DBOperations for specific roles.
/// If operation array is empty, all access to the sensor is removed for the roles.
/// Can only be called with a transaction that should be managed outside.
pub async fn update_sensor_access(sensor_id: uuid::Uuid, permissions: Vec<SensorPermissionRequest>, state: &AppState, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    let sensor = cache::request_sensor(sensor_id, &state).await;
    let current_perm = sensor.map_or(Vec::new(), |s| s.permissions);

    let mut perm_changed = false;
    
    for permission in permissions {
        let role = cache::request_role(permission.role_id, &state).await.expect(&format!("Couldn't find role with name {}!", permission.role_id));

        let mut allow_info = false;
        let mut allow_read = false;
        let mut allow_write = false;

        for op in permission.operations {
            match op {
                DBOperation::INFO => allow_info = true,
                DBOperation::READ => allow_read = true,
                DBOperation::WRITE => allow_write = true
            }
        }

        // Fetch existing permission entry and check if anything changed -> skip otherwise
        
        let ex_perm = current_perm.iter().find(|p| p.role_id == role.id);
        if ex_perm.is_some() {
            let ex_perm = ex_perm.unwrap();
            
            if ex_perm.allow_info == allow_info && ex_perm.allow_read == allow_read && ex_perm.allow_write == allow_write {
                continue;
            }
        }

        perm_changed = true;

        if !allow_info && !allow_read && !allow_write {
            // No permissions left, remove whole permission entry

            let query_res = sqlx::query("DELETE FROM sensor_permissions WHERE sensor_id=$1 AND role_id=$2")
                .bind(sensor_id)
                .bind(role.id)
                .execute(&mut **tx)
                .await
                .map_err(|err: sqlx::Error| err.to_string());

            if let Err(err) = query_res {
                println!("Couldn't remove permissions for sensor id {} and role id {}!", sensor_id, role.id);
                anyhow::bail!(err)
            }
        } else {
            // Insert or update permissions

            let query_res = sqlx::query(r#"
            INSERT INTO sensor_permissions(sensor_id, role_id, allow_info, allow_read, allow_write)
            VALUES($1, $2, $3, $4, $5)
            ON CONFLICT (sensor_id, role_id) 
            DO UPDATE SET 
            allow_info = EXCLUDED.allow_info, 
            allow_read = EXCLUDED.allow_read, 
            allow_write = EXCLUDED.allow_write"#)
                .bind(sensor_id)
                .bind(role.id)
                .bind(allow_info)
                .bind(allow_read)
                .bind(allow_write)
                .execute(&mut **tx)
                .await
                .map_err(|err: sqlx::Error| err.to_string());

            if let Err(err) = query_res {
                println!("Couldn't update permissions for sensor id {} and role id {}!", sensor_id, role.id);
                anyhow::bail!(err)
            }
        }
    }

    // Check if the api keys which are related to this sensor are still valid for their users (VERY EXPENSIVE!)
    
    if perm_changed {
        let new_sensor = get_full_sensor_info(sensor_id, tx.as_mut()).await?;
        
        let _ = validate_api_keys(Some(new_sensor), None, &state, tx.as_mut()).await?;
    }

    Ok(())
}

/* ------------------------------------------------ API Keys ------------------------------------------------------------ */

/// Returns api keys for the specified sensor and the specified user.
pub async fn get_api_keys(sensor_id: uuid::Uuid, user_id: uuid::Uuid, state: &AppState) -> Result<Vec<ApiKey>> {
    let query_res = sqlx::query_as!(ApiKey, "SELECT * FROM api_keys WHERE sensor_id=$1 AND user_id=$2", sensor_id, user_id)
        .fetch_all(&state.db)
        .await;

    match query_res {
        Ok(keys) => Ok(keys),
        Err(err) => {
            println!("Couldn't fetch api_keys for sensor {} and user {}!", sensor_id, user_id);
            anyhow::bail!(err);
        }
    }
}

/// Returns a specific api key by id.
pub async fn get_api_key(key_id: uuid::Uuid, state: &AppState) -> Result<ApiKey> {
    let query_res = sqlx::query_as!(ApiKey, "SELECT * FROM api_keys WHERE id=$1", key_id)
        .fetch_one(&state.db)
        .await;

    match query_res {
        Ok(key) => Ok(key),
        Err(err) => {
            println!("Couldn't fetch api_key with id {}!", key_id);
            anyhow::bail!(err);
        }
    }
}

/// Creates a new api key for the specified user and sensor.
pub async fn create_api_key(sensor_id: uuid::Uuid, user_id: uuid::Uuid, request: CreateApiKeyRequest, state: &AppState) -> Result<ApiKey> {
    if request.operation != DBOperation::READ && request.operation != DBOperation::WRITE {
        anyhow::bail!("Invalid DB operation for api key!");
    }
    
    let new_key = ApiKey {
        id: uuid::Uuid::new_v4(),
        user_id,
        sensor_id,
        name: request.name,
        operation: request.operation,
    };

    let query_result =
        sqlx::query("INSERT INTO api_keys VALUES($1, $2, $3, $4, $5)")
            .bind(&new_key.id)
            .bind(&new_key.user_id)
            .bind(&new_key.sensor_id)
            .bind(&new_key.name)
            .bind(&new_key.operation.as_str())
            .execute(&state.db)
            .await
            .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        println!("Couldn't create api key for sensor id {} and user id {}!", sensor_id, user_id);
        anyhow::bail!(err);
    }

    Ok(new_key)
}

/// Deletes api keys by id.
pub async fn delete_api_keys(keys: Vec<uuid::Uuid>, con: &mut PgConnection, _state: &AppState) -> Result<()> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("DELETE FROM api_keys WHERE id IN ( ");
    
    let mut separated = query_builder.separated(", ");

    for (index, key) in keys.iter().enumerate() {
        separated.push(format!("${}", (index + 1)));
        separated.push_bind(key);
    }

    separated.push_unseparated(")");

    let query = query_builder.build();
    
    let query_result =
        query.execute(&mut *con)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        println!("Couldn't delete api keys {:?}!", keys);
        anyhow::bail!(err)
    }

    for key in keys.iter() {
        cache::purge_api_key(*key, _state);
    }
    
    Ok(())
}

/// Checks if the api keys for the specified user and/or sensor can still be access by the respective users.
/// This function is utilized after user-sensor-permissions where modified in any way (should pass modified user/sensor).
/// Including: Role revoked from user (or role deleted), sensor-permissions modified.
pub async fn validate_api_keys(sensor: Option<FullSensorInfo>, user: Option<UserInfo>, state: &AppState, ex: &mut PgConnection) -> Result<()> {
     // Build query to get all affected keys, depending on input sensor, user

    let query = match (&sensor, &user) {
        (Some(sens), Some(us)) => sqlx::query_as!(ApiKey, "SELECT * FROM api_keys WHERE sensor_id=$1 AND user_id=$2", sens.id, us.id).fetch_all(&mut *ex).await,
        (Some(sens), None) => sqlx::query_as!(ApiKey, "SELECT * FROM api_keys WHERE sensor_id=$1", sens.id).fetch_all(&mut *ex).await,
        (None, Some(us)) => sqlx::query_as!(ApiKey, "SELECT * FROM api_keys WHERE user_id=$1", us.id).fetch_all(&mut *ex).await,
        _ => anyhow::bail!("Invalid parameters for sensor_access_change!")
    };

    // Fetch all desired api keys from DB
    
    let query_res = query.map_err(|err: sqlx::Error| err.to_string());
    
    if let Err(err) = query_res {
        println!("Couldn't fetch api keys for sensor {:?} and user {:?}!", sensor, user);
        anyhow::bail!(err)
    }

    // Check for each key, if the user can still access the sensor (Might be very expensive for many existing keys!)
    // This partially fetches data from cache or takes the provided user/sensor.
    // The latter case is important in case super function call has modified user/sensor data.
    
    let mut keys_to_delete: Vec<uuid::Uuid> = Vec::new();

    for key in query_res.unwrap() {
        let u = match &user {
            Some(us) => Some(us.to_owned()),
            None => cache::request_user(key.user_id, &state).await
        };

        if u.is_none() {
            anyhow::bail!("Couldn't get user {} for api_key {}!", key.user_id, key.id)
        }

        let s = match &sensor {
            Some(us) => Some(us.to_owned()),
            None => cache::request_sensor(key.sensor_id, &state).await
        };

        if s.is_none() {
            anyhow::bail!("Couldn't get sensor {} for api_key {}!", key.sensor_id, key.id)
        }
        
        let permissions = get_user_sensor_permissions_impl(u.as_ref(), s.as_ref().unwrap(), &state).await;

        let perm = match key.operation {
            DBOperation::READ => UserSensorPerm::Read,
            DBOperation::WRITE => UserSensorPerm::Write,
            _ => anyhow::bail!("Invalid operation for api_key {}!", key.id)
        };

        if !permissions.has(perm) {
            keys_to_delete.push(key.id);
        }
    }

    // Remove invalidated keys (if any)
    
    if !keys_to_delete.is_empty() {
        let res = delete_api_keys(keys_to_delete.clone(), &mut *ex, state).await;
        
        if res.is_err() {
            println!("Couldn't delete API keys {:?}!", keys_to_delete);
            anyhow::bail!("Error on deleting api keys during permission edit!")
        }
    }

    Ok(())
 }