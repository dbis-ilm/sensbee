use crate::{
    database::{models::user::User, user_db::login_from_oauth},
    features::config::{get_external_host, ServerConfig},
    handler::models::responses::AuthResponse,
    state::AppState,
    utils::generate_random_string,
};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::reqwest;
use openidconnect::{
    AccessTokenHash, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, RedirectUrl, Scope,
};
use openidconnect::{OAuth2TokenResponse, TokenResponse};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::RwLock};
use tracing::{error, info};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct OIDCInternalID(u32);

// Internal representation of an OIDC
#[derive(Debug, Clone)]
pub struct OIDCInternal {
    pub id: OIDCInternalID, // this is just used for runtime references! these might be different between app starts of the same effective IDP
    pub name: String,
    pub issuer: String,
    pub client_id: ClientId,
    pub client_secret: ClientSecret,
    pub provider_metadata: Option<CoreProviderMetadata>, // Existence implies that this OIDC is configured correctly and can be used
}

// Used to track the state of an individual authentication flow
#[derive(Debug, Clone)]
pub struct AuthFlowState {
    ref_id: OIDCInternalID,
    nonce: Nonce,
    state: CsrfToken,
}

// Highest level of state representation for the OIDC authentication mechanism
#[derive(Debug)]
pub struct OAuthState {
    pub configured: RwLock<bool>,
    pub idps: RwLock<Vec<OIDCInternal>>,
    pub flow_states: RwLock<HashMap<String, AuthFlowState>>,
}

pub fn init_oauth(cfg: &ServerConfig) -> OAuthState {
    let mut oidcs = vec![];

    match &cfg.auth {
        Some(auth_cfg) => {
            let mut idx: u32 = 0;
            for idp in &auth_cfg.oidc_clients {
                oidcs.push(OIDCInternal {
                    id: OIDCInternalID(idx),
                    name: idp.name.clone(),
                    issuer: idp.issuer_url.clone(),
                    client_id: ClientId::new(idp.client_id.clone()),
                    client_secret: ClientSecret::new(idp.client_secret.clone()),
                    provider_metadata: None,
                });
                info!("IDP: '{}' found", idp.name.clone());
                idx += 1;
            }
        }
        None => {}
    };

    OAuthState {
        configured: RwLock::new(false),
        idps: RwLock::new(oidcs),
        flow_states: RwLock::new(HashMap::new()),
    }
}

// This is the user facing representation of an available OIDC
#[derive(Debug, Serialize, Deserialize)]
pub struct IDP {
    pub name: String,
    pub final_url: String,
}

impl IDP {
    pub fn from_internal(internal: &OIDCInternal, final_url: String) -> IDP {
        IDP {
            name: internal.name.clone(),
            final_url,
        }
    }
}

fn redirect_url(state: AppState) -> String {
    format!("{}/auth/openid/callback", get_external_host(&state.cfg))
}

// ##############################
// Init available OIDC from config
//
// Should only be called once during boot. We dont support changing this during runtime.

pub async fn setup_available_idp(state: &AppState) -> anyhow::Result<()> {
    // Check if configuration has already happened
    match state.oauth.configured.try_read() {
        Err(_) => {
            // Another task is currently setting up the IDPs
            return Ok(());
        }
        Ok(g) => {
            if *g {
                return Ok(());
            }
        }
    };

    // We need to configure. First grab the write lock on the config
    let mut g_lock = state.oauth.configured.write().unwrap();
    // Keep this until the end!

    let mut idps_g = state.oauth.idps.write().unwrap();

    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    for idp in idps_g.iter_mut() {
        // Create the url that will be queried for its openid configuration
        let url = match IssuerUrl::new(idp.issuer.clone()) {
            Ok(v) => v,
            Err(err) => {
                error!(
                    "IDP: '{}' IssuerUrl::new failed with: {}",
                    idp.name.clone(),
                    err
                );
                continue;
            }
        };

        // Use OpenID Connect Discovery to fetch the provider metadata.
        let m = match CoreProviderMetadata::discover_async(url, &http_client).await {
            Ok(d) => d,
            Err(err) => {
                error!(
                    "IDP: '{}' at '{}' discovery failed with: {:?}",
                    idp.name.clone(),
                    idp.issuer.clone(),
                    err
                );
                continue;
            }
        };

        idp.provider_metadata = Some(m);

        info!("IDP: '{}' configured successfully", idp.name.clone());
    }

    *g_lock = true;

    Ok(())
}

// ##############################
// Retrieve the list of available OIDC and set them up to be available for logging in
//
// Note: Not sure if this is needed to be done before the user interacts with the endpoint.
// We could also just return the list without setup, and then use a redirect when a button is clicked
// to generate the required data once we know which one will be used...
pub async fn list_available_idp(state: &AppState) -> Vec<IDP> {
    // Make sure that we already configured the IDP list
    let _ = setup_available_idp(state).await;

    let mut res = vec![];

    // Copy the discovered IPDs
    let g = state.oauth.idps.read().unwrap();
    let idps = g.clone();
    drop(g);

    // Iterate all the IDPs that we have and return those that have a valid config
    for idp in idps {
        // Skip those without metadata, they are not correctly configured, thus unavailable
        let m = match &idp.provider_metadata {
            Some(m) => m,
            None => continue,
        };

        let client = CoreClient::from_provider_metadata(
            m.clone(),
            idp.client_id.clone(),
            Some(idp.client_secret.clone()),
        )
        // Set the URL the user will be redirected to after the authorization process.
        .set_redirect_uri(RedirectUrl::new(redirect_url(state.clone())).unwrap());

        // Generate the full authorization URL.
        let (auth_url, csrf_token, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".to_owned()))
            .add_scope(Scope::new("email".to_owned()))
            .url();

        let mut g = state.oauth.flow_states.write().unwrap();
        g.insert(
            csrf_token.clone().into_secret(),
            AuthFlowState {
                ref_id: idp.id,
                nonce,
                state: csrf_token,
            },
        );

        res.push(IDP::from_internal(&idp, auth_url.to_string()));
    }

    return res;
}

// ##############################
// After the OIDC has come to a result it will be examind in here.
//
pub async fn retrieve_user_info(resp: AuthResponse, app_state: &AppState) -> anyhow::Result<User> {
    // Ensure that we have a success response
    if let AuthResponse::Error {
        error,
        error_description,
        state: _,
    } = resp
    {
        error!("{error}: {error_description:?}");
        anyhow::bail!(error);
    }
    let AuthResponse::Success { code, state } = resp else {
        anyhow::bail!("Failed to parse request params as any AuthResponse");
    };

    // Grab the flow state
    let g = app_state.oauth.flow_states.read().unwrap();
    let r = match g.get(&state.clone().into_secret()) {
        Some(e) => e.clone(),
        None => anyhow::bail!("{:?} not in {:?}", state, g),
    };
    drop(g);

    // Make sure that CSRF Token match
    if r.state.into_secret() != state.into_secret() {
        anyhow::bail!("csrf state mismatch");
    }

    // Grab the OIDC that was used
    let g = app_state.oauth.idps.read().unwrap();
    let idp = g.iter().find(|idp| idp.id == r.ref_id).unwrap().clone();
    drop(g);

    let redirect_url_string = match RedirectUrl::new(redirect_url(app_state.clone())) {
        Ok(v) => v,
        Err(err) => anyhow::bail!("RedirectUrl::new() failed with: {}", err),
    };

    // Create the client data structure for the token exchange to retrieve the user data
    let client = CoreClient::from_provider_metadata(
        idp.provider_metadata.unwrap(),
        idp.client_id.clone(),
        Some(idp.client_secret.clone()),
    )
    .set_redirect_uri(redirect_url_string);
    let http_client = match reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(v) => v,
        Err(err) => anyhow::bail!("reqwest::ClientBuilder::new() failed with: {}", err),
    };

    let code_exchange = match client.exchange_code(code) {
        Ok(v) => v,
        Err(err) => anyhow::bail!("client.exchange_code() failed with: {}", err),
    };

    let token = match code_exchange.request_async(&http_client).await {
        Ok(v) => v,
        Err(err) => anyhow::bail!("code_exchange.request_async() failed with: {:?}", err),
    };

    // Extract the ID token claims after verifying its authenticity and nonce.
    let id_token = match token.id_token() {
        Some(v) => v,
        None => anyhow::bail!("Server response without id_token"),
    };
    let id_token_verifier = client.id_token_verifier();
    let claims = match id_token.claims(&id_token_verifier, &r.nonce) {
        Ok(v) => v,
        Err(err) => anyhow::bail!("id_token.claims() failed with {}", err),
    };

    // Verify the access token hash to ensure that the access token hasn't been substituted for
    // another user's.
    if let Some(expected_access_token_hash) = claims.access_token_hash() {
        let actual_access_token_hash = AccessTokenHash::from_token(
            token.access_token(),
            id_token.signing_alg()?,
            id_token.signing_key(&id_token_verifier)?,
        )?;
        if actual_access_token_hash != *expected_access_token_hash {
            anyhow::bail!("Invalid access token");
        }
    }

    // Everything seems to have worked

    // If we didnt get an email address we generate a random address
    let n = match claims.email().map(|email| email.as_str()) {
        Some(v) => v,
        None => {
            let random_name = format!("{}@sensbee.local", generate_random_string(8)).to_string();
            &random_name.clone()
        }
    };

    // Now retrieve the associated sensbee user
    login_from_oauth(n, &idp.issuer, claims.subject(), &app_state).await
}
