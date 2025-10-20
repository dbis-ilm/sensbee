use crate::database::models::role::{self, ROLE_SYSTEM_USER};
use crate::database::models::role::{ROLE_SYSTEM_ADMIN, ROLE_SYSTEM_ROOT};
use crate::database::models::user::*;
use crate::database::role_db::{self, assign_role};
use crate::features::cache;
use crate::features::config::{new_users_default_verified, root_user_email};
use crate::handler::models::requests::RegisterUserRequest;
use crate::{
    database::models::user::User, handler::models::requests::EditUserInfoRequest, state::AppState,
};
use anyhow::Ok;
use anyhow::Result;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// Create a new user and insert into the database. Fails if a user
/// with the same email exists already.
pub async fn register_user(user_info: RegisterUserRequest, state: &AppState) -> Result<User> {
    let exists = user_exists(user_info.email.clone(), &state).await;

    if exists {
        anyhow::bail!("User with email {} already exists!", user_info.email)
    }

    let mut tx = state.db.begin().await?;

    // insert user record into table
    let user_res =
        sqlx::query_as::<_, User>("INSERT INTO users (name, email) VALUES ($1, $2) RETURNING *")
            .bind(user_info.name.to_string())
            .bind(user_info.email.to_string())
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

        println!(
            "Couldn't assign user role to user {} during register!",
            user.id,
        );
        anyhow::bail!(err);
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

    let query_result = sqlx::query(
        r#"
        UPDATE users SET verified=true WHERE id = $1"#,
    )
    .bind(user_id)
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

pub async fn edit_user_info(
    user_id: Uuid,
    new_info: EditUserInfoRequest,
    state: &AppState,
) -> Result<()> {
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

    let query_result = sqlx::query(
        r#"
    UPDATE users SET name = $2, email = $3 WHERE id = $1"#,
    )
    .bind(u.id)
    .bind(new_info.name)
    .bind(new_info.email)
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

/// Delete the user with the given id.
pub async fn delete_user(user_id: Uuid, state: &AppState) -> Result<()> {
    let user = cache::request_user(user_id, state).await;

    if user.is_none() {
        anyhow::bail!("Couldn't find user with id {}!", user_id);
    }

    let u = user.unwrap();

    let mut tx = state.db.begin().await?;

    // Deletes user-role entries

    let query_result = sqlx::query(
        r#"
        DELETE FROM user_roles WHERE user_id = $1"#,
    )
    .bind(u.id)
    .execute(&mut *tx)
    .await
    .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // Delete the existing api_keys

    let query_result = sqlx::query("DELETE FROM api_keys WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // Declares all sensors owned by deleted user to system sensors

    let query_result = sqlx::query(
        r#"
        UPDATE sensor SET owner=NULL WHERE owner = $1"#,
    )
    .bind(u.id)
    .execute(&mut *tx)
    .await
    .map_err(|err: sqlx::Error| err.to_string());

    if let Err(err) = query_result {
        let _ = tx.rollback().await;
        anyhow::bail!(err)
    }

    // Now remove the actual user entry

    let query_result = sqlx::query(
        r#"
        DELETE FROM users WHERE email = $1"#,
    )
    .bind(u.email)
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

/// After a successfull oauth flow this will be called to login the authenticated user
/// If no account is found, a new one will be created
/// TODO honor claims for the oauth provider for roles?
pub async fn login_from_oauth(name: &str, iss: &str, sub: &str, state: &AppState) -> Result<User> {
    // Check if we have seen this <iss,sub> pair already!
    let query_result = match sqlx::query_as::<_, UserOnlyId>(
        "SELECT id FROM users_oidc WHERE iss = $1 and sub = $2",
    )
    .bind(iss)
    .bind(sub)
    .fetch_optional(&state.db)
    .await?
    {
        Some(u) => u.id,
        None => {
            // register new user
            // create new user
            let u = match get_user_by_email(name, state).await {
                Err(e) => {
                    anyhow::bail!(e);
                }
                Result::Ok(res) => match res {
                    Some(u) => u,
                    None => {
                        let user_res = sqlx::query_as::<_, User>(
                            "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING *",
                        )
                        .bind(name.to_string())
                        .bind(name.to_string())
                        .fetch_one(&state.db)
                        .await?;

                        assign_role(user_res.id, ROLE_SYSTEM_USER, true, state).await?;

                        user_res
                    }
                },
            };

            // Now that we have a user id save their OpenID id iss and sub tuple
            sqlx::query("INSERT INTO users_oidc(id,iss,sub) VALUES($1,$2,$3)")
                .bind(u.id)
                .bind(iss)
                .bind(sub)
                .execute(&state.db)
                .await?;

            u.id
        }
    };

    let mut user = get_user_by_id(query_result, state).await?;

    if new_users_default_verified(state) && !user.verified {
        verify_user(user.id, state).await?;
        user.verified = true;
    }

    // Check whether we need to set the root role for this new user
    match root_user_email(state) {
        None => {}
        Some(root_name) => {
            if root_name == name {
                if !user.verified {
                    // If this is the root_sub we need to give that role and verify the account!
                    verify_user(user.id, state).await?;
                    user.verified = true;
                }

                // Assign admin role if not already assigned
                if !is_admin_user(user.id, state).await {
                    assign_role(user.id, ROLE_SYSTEM_ADMIN, true, state).await?;
                }

                // Assign root role if not already assigned
                if !is_root_user(user.id, state).await {
                    assign_role(user.id, ROLE_SYSTEM_ROOT, true, state).await?;
                }
            }
        }
    }

    // We need to match our records to the OIDC given values as they might have changed there
    // NOTE we could check if they already match and only update if they dont!
    edit_user_info(
        user.id,
        EditUserInfoRequest {
            name: name.to_owned(),
            email: name.to_owned(),
        },
        state,
    )
    .await?;

    Ok(user)
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
    let query_result = sqlx::query_as::<_, User>(
        r#"
    SELECT id, name, email, verified FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await;

    if let Err(err) = query_result {
        anyhow::bail!(err)
    }

    Ok(query_result?)
}

pub async fn get_user_by_email(email: &str, state: &AppState) -> Result<Option<User>> {
    let query_result = sqlx::query_as::<_, User>(
        r#"
    SELECT id, name, email, verified FROM users WHERE email = $1"#,
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await;

    if let Err(err) = query_result {
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
        .await
        .unwrap_or_default();

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

pub async fn is_root_user(user_id: Uuid, state: &AppState) -> bool {
    let user = cache::request_user(user_id, state).await;

    if user.is_none() {
        return false;
    }

    for ur in user.unwrap().roles.iter() {
        if ur.is_root() {
            return true;
        }
    }

    false
}
