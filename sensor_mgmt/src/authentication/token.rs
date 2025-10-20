use crate::authentication::token_cache;
use jsonwebtoken::errors::ErrorKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const COOKIE_NAME: &str = "sensbee_session";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenDetails {
    pub token: Option<String>,
    pub token_uuid: Uuid,
    pub user_id: Uuid,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub token_uuid: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn generate_jwt_token(
    user_id: Uuid,
    ttl: i64,
    private_key: &[u8],
) -> Result<TokenDetails, anyhow::Error> {
    let now = chrono::Utc::now();
    let mut token_details = TokenDetails {
        user_id,
        token_uuid: Uuid::new_v4(),
        expires_in: Some((now + chrono::Duration::minutes(ttl)).timestamp()),
        token: None,
    };

    let claims = TokenClaims {
        sub: token_details.user_id.to_string(),
        token_uuid: token_details.token_uuid.to_string(),
        exp: token_details.expires_in.unwrap(),
        iat: now.timestamp(),
    };

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_rsa_pem(private_key)?,
    )?;

    token_details.token = Some(token);

    Ok(token_details)
}

pub fn verify_jwt_token(
    public_key: &[u8],
    token: &str,
) -> Result<TokenDetails, jsonwebtoken::errors::Error> {
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);

    let decode_res = jsonwebtoken::decode::<TokenClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_rsa_pem(public_key)?,
        &validation,
    );

    if decode_res.is_err() {
        let err = decode_res.unwrap_err();

        if *err.clone().kind() == ErrorKind::ExpiredSignature {
            // Decode again (ignore exp) and get token id to remove from token_cache

            let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
            validation.validate_exp = false;

            let decoded = jsonwebtoken::decode::<TokenClaims>(
                token,
                &jsonwebtoken::DecodingKey::from_rsa_pem(public_key)?,
                &validation,
            )?;

            let token_uuid = Uuid::parse_str(decoded.claims.token_uuid.as_str()).unwrap();

            token_cache::unregister_token(token_uuid);
        }

        return Err(err);
    }

    let decoded = decode_res?;

    let user_id = Uuid::parse_str(decoded.claims.sub.as_str()).unwrap();
    let token_uuid = Uuid::parse_str(decoded.claims.token_uuid.as_str()).unwrap();

    Ok(TokenDetails {
        token: None,
        token_uuid,
        user_id,
        expires_in: None,
    })
}
