use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;
use once_cell::sync::Lazy;
use crate::authentication::token::TokenDetails;

// Thread-safe token cache across the application

static ACCESS_TOKEN: Lazy<RwLock<HashMap<Uuid, TokenDetails>>> = Lazy::new(RwLock::default);

pub fn has_token(token_id: Uuid) -> (bool, Option<Uuid>) {
    let tokens = ACCESS_TOKEN.read().unwrap();
    
    let token = tokens.get(&token_id);

    (token.is_some(), token.map(|t| t.user_id))
}

pub fn register_token(token_id: Uuid, token: TokenDetails) {
    let mut tokens = ACCESS_TOKEN.write().unwrap();

    tokens.insert(token_id, token);
}

pub fn unregister_token(token_id: Uuid) {
    let mut tokens = ACCESS_TOKEN.write().unwrap();
    
    tokens.remove(&token_id);
}

pub fn get_token_by_string(t: String) -> Option<TokenDetails> {
    let tokens = ACCESS_TOKEN.read().unwrap();
    
    for token in tokens.values() {
        if token.token.clone().unwrap() == t {
            return Some(token.clone())
        }
    }
    
    None
}