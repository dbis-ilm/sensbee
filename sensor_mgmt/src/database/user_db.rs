use anyhow::Result;
use sqlx::{PgConnection, Row};
use uuid::Uuid;
use crate::{database::models::user::User, handler::models::requests::{EditUserPasswordRequest, EditUserInfoRequest}, state::AppState};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use crate::database::models::role;
use crate::database::models::user::*;
use crate::database::role_db;
use crate::features::cache;
use crate::handler::models::requests::{LoginUserRequest, RegisterUserRequest};
/// Create a new user with password and insert into the database. Fails if a user
/// with the same email exists already.
pub async fn register_user(user_info: RegisterUserRequest, admin: bool, state: &AppState) -> Result<User> {
    let exists= user_exists(user_info.email.clone(), &state).await;

    if exists {
        anyhow::bail!("User with email {} already exists!", user_info.email)
    }

    let mut tx = state.db.begin().await?;

    // insert user record into table
    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = Argon2::default()
        .hash_password(user_info.password.as_bytes(), &salt)
        .expect("Error while hashing password")
        .to_string();
    let user_res = sqlx::query_as!(
            User,
            "INSERT INTO users (name, email, password) VALUES ($1, $2, $3) RETURNING *",
            user_info.name.to_string(),
            user_info.email.to_string(),
            hashed_password
        )
    .fetch_one(&mut *tx)
    .await
    .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = user_res {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    let user = user_res.unwrap();

    // ----- Add user role -----

    let user_role = cache::request_role(role::ROLE_SYSTEM_USER, &state).await;

    if user_role.is_none() {
        let _ = tx.rollback().await;
        anyhow::bail!("Couldn't fetch user role for user {}!", user.id);
    }

    let res = role_db::assign_role_by_id(user.id, user_role.unwrap().id, tx.as_mut(), state).await;

    if let Err(err) = res {
        let _ = tx.rollback().await;
        
        println!("Couldn't assign user role to user {} during register!", user.id,);
        anyhow::bail!(err);
    }

    // ----- Add admin role if required -----

    if admin {
        let admin_role = cache::request_role(role::ROLE_SYSTEM_ADMIN, &state).await;

        if admin_role.is_none() {
            let _ = tx.rollback().await;
            anyhow::bail!("Couldn't fetch admin role for user {}!", user.id);
        }

        let res = role_db::assign_role_by_id(user.id, admin_role.unwrap().id, tx.as_mut(), state).await;

        if let Err(err) = res {
            let _ = tx.rollback().await;

            println!("Couldn't assign admin role to user {} during register!", user.id,);
            anyhow::bail!(err);
        }
    }

    let _ = tx.commit().await;

    Ok(user)
}

/// Verifies an existing user with the given id.
pub async fn verify_user(user_id: Uuid, state: &AppState) -> Result<()> {
    let user = cache::request_user(user_id, state).await;

    if user.is_none() {
        anyhow::bail!("Couldn't find user with id {}!", user_id);
    }
    
    let query_result = sqlx::query!(r#"
        UPDATE users SET verified=true WHERE id = $1"#, user_id)
        .execute(&state.db)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        println!("Couldn't verify user with id {}!", user_id);
        anyhow::bail!(err)
    }
    
    cache::purge_user(user_id, state);
    
    Ok(())
}

pub async fn edit_user_info(user_id: Uuid, new_info: EditUserInfoRequest, state: &AppState)-> Result<()>{

    // Input validation

    if new_info.name.len() == 0 {
        anyhow::bail!("new name cant be empty");
    }
    if new_info.email.len() == 0 {
        anyhow::bail!("new email cant be empty");
    }
    // TODO validate that this is a valid email? 

    let user = cache::request_user(user_id, state).await;
    if user.is_none() {
        anyhow::bail!("Couldn't find user with id {}!", user_id);
    }

    let u = user.unwrap();

    let mut tx = state.db.begin().await?;

    let query_result = sqlx::query!(r#"
    UPDATE users SET name = $2, email = $3 WHERE id = $1"#, u.id,new_info.name,new_info.email)
    .execute(&mut *tx)
    .await
    .map_err(|err: sqlx::Error| err.to_string());
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    let _ = tx.commit().await;

    cache::purge_user(user_id, state);

    Ok(())
}

pub async fn update_user_password(editor_user_id: Uuid, target_user_id: Uuid, pws: EditUserPasswordRequest, state: &AppState)-> Result<()>{
    let user = cache::request_user(target_user_id, state).await;
    if user.is_none() {
        anyhow::bail!("Couldn't find user with id {}!", target_user_id);
    }

    // A user may edit their own password providing their old password
    // An admin can change the password of any other user

    let validated_pw = pws.new.trim();

    // TODO make this configuarble?
    let min_pw_len = 8;
    if validated_pw.len() < min_pw_len {
        anyhow::bail!("password must be at lest {} characters", min_pw_len)
    }

    let u = user.unwrap();

    let mut tx = state.db.begin().await?;

    let admin = is_admin_user(editor_user_id, state).await;
    if !admin {
        if pws.old.len() == 0 {
            anyhow::bail!("Missing old password");
        }
        // verify old password matches saved hash
        let query_result = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", target_user_id)
                .fetch_optional(&state.db)
                .await
                .unwrap();
        let is_valid = query_result.to_owned().map_or(false, |user| {
            let parsed_hash = PasswordHash::new(&user.password).unwrap();
            Argon2::default()
                .verify_password(pws.old.as_bytes(), &parsed_hash)
                .map_or(false, |_| true)
        });
        if !is_valid {
            anyhow::bail!("Invalid old password");
        }
    }
    
    // Generate new hash based on the new and validated password
    let salt = SaltString::generate(&mut OsRng);
    let hashed_new_password = Argon2::default()
        .hash_password(validated_pw.as_bytes(), &salt)
        .expect("Error while new hashing password")
        .to_string();

    let query_result = sqlx::query!(r#"
    UPDATE users SET password = $2 WHERE id = $1"#, u.id, hashed_new_password)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    let _ = tx.commit().await;

    cache::purge_user(target_user_id, state);

    Ok(())
}

/// Delete the user with the given id.
pub async fn delete_user(user_id: Uuid, state: &AppState) -> Result<()> {
    let user = cache::request_user(user_id, state).await;

    if user.is_none() {
        anyhow::bail!("Couldn't find user with id {}!", user_id);
    }

    let u = user.unwrap();

    let mut tx = state.db.begin().await?;

    // Deletes user-role entries
    
    let query_result = sqlx::query!(r#"
        DELETE FROM user_roles WHERE user_id = $1"#, u.id)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // Delete the existing api_keys
    
    let query_result = sqlx::query!("DELETE FROM api_keys WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());
    
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }
    
    // Declares all sensors owned by deleted user to system sensors

    let query_result = sqlx::query!(r#"
        UPDATE sensor SET owner=NULL WHERE owner = $1"#, u.id)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }
    
    // Now remove the actual user entry
    
    let query_result = sqlx::query!(r#"
        DELETE FROM users WHERE email = $1"#, u.email)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());
    
    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }
    
    let _ = tx.commit().await;
    
    cache::purge_user(u.id, state);
    cache::purge_sensors_owned_by(u.id, state);
    cache::purge_api_keys_for_user(u.id, state);

    Ok(())
}

/// Check if a user with the given email exists and has the same password.
pub async fn check_user_login(user_info: LoginUserRequest, state: &AppState) -> Result<User> {
    let query_result = sqlx::query_as!(User, "SELECT * FROM users WHERE email = $1", user_info.email)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    let is_valid = query_result.to_owned().map_or(false, |user| {
        let parsed_hash = PasswordHash::new(&user.password).unwrap();
        Argon2::default()
            .verify_password(user_info.password.as_bytes(), &parsed_hash)
            .map_or(false, |_| true)
    });

    if !is_valid {
        anyhow::bail!("Invalid email or password");
    }
    
    Ok(query_result.unwrap())
}

/// Check if a user with the given email exists.
pub async fn user_exists(email: String, state: &AppState) -> bool {
    let exists: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(email.to_owned())
        .fetch_one(&state.db)
        .await
        .unwrap()
        .get(0);

    exists
}

pub async fn get_user_by_id(user_id: Uuid, state: &AppState) -> Result<User> {
    let mut con = state.db.begin().await?;
    
    let res = get_user_by_id_impl(user_id, con.as_mut()).await;
    
    let _ = con.commit().await;
    
    res
}

/// Return the information for the user with the given user_id.
pub async fn get_user_by_id_impl(user_id: Uuid, conn: &mut PgConnection) -> Result<User> {
    let query_result = sqlx::query_as!(User, r#"
    SELECT id, name, email, password, verified FROM users WHERE id = $1"#, user_id)
        .fetch_one(&mut *conn)
        .await;

    if let Err(err) = query_result {
        println!("Couldn't retrieve user with id {}", user_id);
        anyhow::bail!(err)
    }

    Ok(query_result?)
}

/// Fetches information about a specific user including roles.
pub async fn get_user_info(user_id: Uuid, conn: &mut PgConnection) -> Result<UserInfo> {
    let user = get_user_by_id_impl(user_id, conn).await;

    if let Err(err) = user {
        println!("Couldn't find user with id {}", user_id);
        anyhow::bail!(err)
    }
    
    let user = user?;

    let roles = role_db::get_user_roles(user.id, conn).await;

    if let Err(err) = roles {
        println!("Couldn't retrieve roles for user {}!", user.id);
        anyhow::bail!(err)
    }

    let user_entry = UserInfo {
        id: user.id,
        name: user.name,
        email: user.email,
        verified: user.verified,
        roles: roles?,
    };

    Ok(user_entry)
}

/// Return a list of all registered users.
pub async fn user_list(state: &AppState) -> Result<Vec<UserInfo>> {
    let users: Vec<Uuid> = sqlx::query_scalar(r#"SELECT id FROM users"#)
        .fetch_all(&state.db)
        .await.unwrap_or_default();

    let mut res: Vec<UserInfo> = Vec::new();
    
    let mut con = state.db.begin().await?;

    for user_id in users {
        let user_info = get_user_info(user_id, &mut con).await;
        
        if user_info.is_err() {
            println!("Couldn't get user info with id {}", user_id);
            continue;
        }
        
        res.push(user_info?);
    } 
    
    let _ = con.commit().await;
    
    Ok(res)
}

/* ------------------------------------------------ Permission Management ------------------------------------------------------------ */

/// Check if the user with the given user_id exists and has the admin role.
pub async fn is_admin_user(user_id: Uuid, state: &AppState) -> bool {
    let user = cache::request_user(user_id, state).await;
    
    if user.is_none() {
        return false;
    }
    
    for ur in user.unwrap().roles.iter() {
        if ur.is_admin() {
            return true;
        }
    }
    
    false
}
