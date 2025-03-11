use anyhow::Result;
use sqlx::{PgConnection, PgExecutor};
use crate::features::cache;
use crate::database::models::role::Role;
use crate::database::{sensor_db, user_db};
use crate::state::AppState;

pub async fn create_role(role_name: String, state: &AppState) -> Result<Role> {

    // create a new UUID
    let role_id = uuid::Uuid::new_v4();

    // NOTE system roles should only be created using the cli tool. The tool talks directly to the DB. 
    let new_role = sqlx::query_as!(Role, "INSERT INTO roles(id, name) VALUES($1, $2) RETURNING *", role_id, role_name.to_string())
        .fetch_one(&state.db)
        .await;

    match new_role {
        Ok(role) => Ok(role),
        Err(err) => {
            println!("Creation of new role with name {} failed!", role_name);
            anyhow::bail!(err);
        }
    }
}

pub async fn delete_role(role_id: uuid::Uuid, allow_system_delete: bool, state: &AppState) -> Result<()> {
    // Fetch role and verify that it can be removed
    let role = cache::request_role(role_id, &state).await;

    if role.is_none() {
        anyhow::bail!("Role with name {} not found!", role_id);
    }

    let role = role.unwrap();

    if role.system && !allow_system_delete{
        anyhow::bail!("Can't delete system roles!");
    }

    let mut tx = state.db.begin().await?;

    // Delete the sensor_permission entries for this role

    let sensor_res = sqlx::query("DELETE FROM sensor_permissions WHERE role_id = $1")
        .bind(role.id)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = sensor_res {
        let _ = tx.rollback().await;
        println!("Deletion of sensor_permissions for role id {} failed!", role.id);
        anyhow::bail!(err)
    }

    // Delete the user_role entries for this role

    let user_res = sqlx::query_scalar!("DELETE FROM user_roles WHERE role_id = $1 RETURNING user_id", role.id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = user_res {
        let _ = tx.rollback().await;
        println!("Deletion of user_roles for role id {} failed!", role.id);
        anyhow::bail!(err)
    }

    let affected_users = user_res.unwrap();

    // Finally, delete the actual role

    let role_res = sqlx::query("DELETE FROM roles WHERE id = $1")
        .bind(role.id)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = role_res {
        let _ = tx.rollback().await;
        println!("Deletion of role with id {} failed!", role.id);
        anyhow::bail!(err)
    }

    // Check for all affected users if their api keys are still valid (VERY EXPENSIVE!)
    
    for u in affected_users.iter() {
        // Get new user entry since his role changed
        let user_res = user_db::get_user_info(*u, tx.as_mut()).await;

        if let Err(err) = user_res {
            let _ = tx.rollback().await;
            println!("Couldn't fetch user {} from DB!", *u);
            anyhow::bail!(err)
        }

        // Check if the api keys of the user are still valid after revoking the role from him!

        let res = sensor_db::validate_api_keys(None, Some(user_res?), &state, tx.as_mut()).await;

        if let Err(err) = res {
            let _ = tx.rollback().await;
            println!("Couldn't validate api keys for user {}!", *u);
            anyhow::bail!(err)
        }
    }

    let _ = tx.commit().await;
    
    for u in affected_users {
        cache::purge_user(u, state);
    }   
    cache::purge_role(role.id, &state);

    Ok(())
}

/// Gets the role entry based on its name from the db.
pub async fn get_role(id: uuid::Uuid, state: &AppState) -> Result<Role> {
    let role = sqlx::query_as!(Role, "SELECT * FROM roles WHERE id = $1", id)
        .fetch_one(&state.db)
        .await;

    match role {
        Ok(role) => Ok(role),
        Err(err) => {
            println!("Role with name {} not found!", id);
            anyhow::bail!(err)
        }
    }
}

/// Assigns a role to a user by the name of the role. This fetches the respective role entry from db first.
pub async fn assign_role(user_id: uuid::Uuid, role_id: uuid::Uuid, allow_system: bool, state: &AppState) -> Result<()> {
    let user = cache::request_user(user_id, state).await;

    if user.is_none() {
        anyhow::bail!("Couldn't find user with id {}!", user_id);
    }
    
    // Request the role from DB by its name
    let role = cache::request_role(role_id, &state).await;

    if role.is_none() {
        anyhow::bail!("Couldn't find role with name {}!", role_id);
    }

    let role = role.unwrap();

    if role.system && !allow_system {
        anyhow::bail!("System role {} can't be assigned to user {}!", role_id, user_id);
    }

    let res = assign_role_by_id(user_id, role.id, &state.db, state).await;

    if let Err(err) = res {
        println!("Couldn't assign role {} to user {}!", role_id, user_id);
        anyhow::bail!(err)
    }

    Ok(())
}

/// Assigns a role by its id to a user, can be used during a transaction by passing the transaction obj.
/// This also allows to assign system roles, e.g. when users are created.
pub async fn assign_role_by_id(user_id: uuid::Uuid, role_id: uuid::Uuid, executor: impl PgExecutor<'_>, _state: &AppState) -> Result<()> {
    let query_res = sqlx::query("INSERT INTO user_roles(user_id, role_id) VALUES($1, $2)")
        .bind(user_id)
        .bind(role_id)
        .execute(executor)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_res {
        println!("Couldn't assign role {} to user {}!", role_id, user_id);
        anyhow::bail!(err)
    }

    cache::purge_user(user_id, _state);
    
    Ok(())
}

/// Revokes a role by its name from a user. System roles can not be removed manually from users!
pub async fn revoke_role(user_id: uuid::Uuid, role_id: uuid::Uuid, allow_system: bool, state: &AppState) -> Result<()> {
    let user = cache::request_user(user_id, state).await;

    if user.is_none() {
        anyhow::bail!("Couldn't find user with id {}!", user_id);
    }
    
    let role = cache::request_role(role_id, &state).await;
    
    if role.is_none() {
        anyhow::bail!("Couldn't find role with name {}!", role_id);
    }
    
    let role = role.unwrap();
    
    if role.system && !allow_system {
        anyhow::bail!("System role {} can't be revoked from user {}!", role_id, user_id);
    }

    let mut tx = state.db.begin().await?;
    
    let query_res = sqlx::query("DELETE FROM user_roles WHERE user_id=$1 AND role_id=$2")
        .bind(user_id)
        .bind(role.id)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_res {
        let _ = tx.rollback().await;
        println!("Couldn't revoke role {} from user {}!", role.id, user_id);
        anyhow::bail!(err)
    }
    
    // Get new user object from DB since we modified his roles

    let user_res = user_db::get_user_info(user_id, tx.as_mut()).await;

    if let Err(err) = user_res {
        let _ = tx.rollback().await;
        println!("Couldn't fetch user {} from DB!", user_id);
        anyhow::bail!(err)
    }

    // Check if the api keys of the user are still valid after revoking the role from him!

    let res = sensor_db::validate_api_keys(None, Some(user_res?), &state, tx.as_mut()).await;

    if let Err(err) = res {
        let _ = tx.rollback().await;
        println!("Couldn't validate api keys for user {}!", user_id);
        anyhow::bail!(err)
    }

    let _ = tx.commit().await;

    cache::purge_user(user_id, state);

    Ok(())
}

pub async fn list_roles(state: &AppState) -> Result<Vec<Role>> {
    let query_res = sqlx::query_as!(Role, "SELECT * FROM roles")
        .fetch_all(&state.db)
        .await;

    match query_res {
        Ok(roles) => Ok(roles),
        Err(err) => {
            println!("Couldn't fetch roles!");
            anyhow::bail!(err);
        }
    }
}

pub async fn get_user_roles(user_id: uuid::Uuid, conn: &mut PgConnection) -> Result<Vec<Role>> {
    let query_res = sqlx::query_as!(Role, "SELECT roles.id, roles.name, roles.system FROM roles, user_roles WHERE user_roles.user_id=$1 AND user_roles.role_id = roles.id", user_id)
        .fetch_all(conn)
        .await;

    match query_res {
        Ok(roles) => Ok(roles),
        Err(err) => {
            println!("Couldn't fetch roles for user {}!", user_id);
            anyhow::bail!(err);
        }
    }
}
