//! OAuth2 + bearer/API-key resolution and org-scoped request context.

use crate::state::AppState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Json, extract::Query, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thine_platform::{ApiToken, DEMO_ORG_ID, OnboardingGuide};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct OrgCtx {
    pub org_id: String,
    pub org_name: String,
    pub user_email: Option<String>,
    pub roles: Vec<String>,
    pub ingest_api_key: Option<String>,
    pub is_demo: bool,
}

impl OrgCtx {
    pub fn demo() -> Self {
        Self {
            org_id: DEMO_ORG_ID.into(),
            org_name: "Demo workspace".into(),
            user_email: None,
            roles: vec!["viewer".into()],
            ingest_api_key: None,
            is_demo: true,
        }
    }
}

pub fn resolve_org(state: &AppState, headers: &HeaderMap) -> OrgCtx {
    if let Some(org_id) = resolve_ingest_org(state, headers) {
        if let Some(org) = state.platform.tenants.org(&org_id) {
            let key = state.platform.tenants.ingest_key_for_org(&org_id);
            return OrgCtx {
                org_id: org.id.clone(),
                org_name: org.name.clone(),
                user_email: None,
                roles: vec!["ingest".into()],
                ingest_api_key: key,
                is_demo: org_id == DEMO_ORG_ID,
            };
        }
    }
    if let Some(token) = bearer(headers) {
        if let Some(tok) = state.platform.authenticate(token) {
            let key = state
                .platform
                .tenants
                .ingest_key_for_org(&tok.org_id);
            return OrgCtx {
                org_id: tok.org_id.clone(),
                org_name: tok.org_name.clone(),
                user_email: Some(tok.user_email.clone()),
                roles: tok.roles.clone(),
                ingest_api_key: key.or(Some(tok.ingest_api_key.clone())),
                is_demo: tok.org_id == DEMO_ORG_ID,
            };
        }
    }
    OrgCtx::demo()
}

pub fn require_user_org(state: &AppState, headers: &HeaderMap) -> Result<OrgCtx, Response> {
    let ctx = resolve_org(state, headers);
    if ctx.user_email.is_some() {
        return Ok(ctx);
    }
    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "Sign in required", "login": "/api/v1/auth/oauth/google/start" })),
    )
        .into_response())
}

fn resolve_ingest_org(state: &AppState, headers: &HeaderMap) -> Option<String> {
    for name in ["dd-api-key", "x-api-key", "thine-api-key"] {
        if let Some(v) = headers.get(name).and_then(|h| h.to_str().ok()) {
            if let Some(org) = state.platform.tenants.resolve_api_key(v.trim()) {
                return Some(org);
            }
        }
    }
    None
}

pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(str::trim))
}

pub fn site_url(headers: &HeaderMap) -> String {
    if let Ok(u) = std::env::var("THINE_PUBLIC_URL") {
        if !u.is_empty() {
            return u.trim_end_matches('/').into();
        }
    }
    if let Some(host) = headers.get("x-forwarded-host").and_then(|h| h.to_str().ok()) {
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("https");
        return format!("{proto}://{host}");
    }
    if let Some(host) = headers.get(axum::http::header::HOST).and_then(|h| h.to_str().ok()) {
        return format!("http://{host}");
    }
    "http://127.0.0.1:4318".into()
}

#[derive(Debug, Deserialize)]
pub struct SignupReq {
    pub email: String,
    #[serde(default)]
    pub org_name: Option<String>,
    #[serde(default = "default_roles")]
    pub roles: Vec<String>,
}

fn default_roles() -> Vec<String> {
    vec!["admin".into()]
}

pub async fn signup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SignupReq>,
) -> impl IntoResponse {
    let token = state.platform.issue_token_for_customer(
        &req.email,
        req.org_name.as_deref(),
        req.roles,
    );
    state
        .platform
        .tenants
        .advance_onboarding(&token.org_id, "welcome");
    let guide = onboarding_for_token(&state, &headers, &token);
    (
        StatusCode::CREATED,
        Json(json!({ "session": token, "onboarding": guide })),
    )
}

pub async fn onboarding(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let ctx = resolve_org(&state, &headers);
    if ctx.is_demo && ctx.user_email.is_none() {
        return (
            StatusCode::OK,
            Json(json!({
                "demo": true,
                "message": "Sign in to get your own workspace and ingest key.",
                "oauth_start": "/api/v1/auth/oauth/google/start",
                "signup": "/api/v1/auth/signup",
            })),
        )
            .into_response();
    }
    let mut guide = state
        .platform
        .tenants
        .onboarding_guide(&ctx.org_id, &site_url(&headers))
        .unwrap_or_else(|| OnboardingGuide {
            org_id: ctx.org_id.clone(),
            org_name: ctx.org_name.clone(),
            ingest_api_key: ctx.ingest_api_key.clone().unwrap_or_default(),
            site_url: site_url(&headers),
            onboarding: Default::default(),
            steps: vec![],
        });
    if state.platform.list_fleet_for(&ctx.org_id).is_empty() {
        // keep agent step open
    } else {
        state
            .platform
            .tenants
            .advance_onboarding(&ctx.org_id, "agent");
        if let Some(g) = state
            .platform
            .tenants
            .onboarding_guide(&ctx.org_id, &site_url(&headers))
        {
            guide = g;
        }
    }
    Json(guide).into_response()
}

#[derive(Debug, Deserialize)]
pub struct OnboardingPatch {
    pub step: String,
}

pub async fn onboarding_advance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<OnboardingPatch>,
) -> impl IntoResponse {
    let ctx = match require_user_org(&state, &headers) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let ob = state
        .platform
        .tenants
        .advance_onboarding(&ctx.org_id, &req.step)
        .unwrap_or_default();
    Json(json!({ "onboarding": ob })).into_response()
}

fn onboarding_for_token(
    state: &AppState,
    headers: &HeaderMap,
    token: &ApiToken,
) -> OnboardingGuide {
    state
        .platform
        .tenants
        .onboarding_guide(&token.org_id, &site_url(headers))
        .unwrap_or_else(|| OnboardingGuide {
            org_id: token.org_id.clone(),
            org_name: token.org_name.clone(),
            ingest_api_key: token.ingest_api_key.clone(),
            site_url: site_url(headers),
            onboarding: Default::default(),
            steps: vec![],
        })
}

pub async fn oauth_google_start(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let client_id = std::env::var("THINE_OAUTH_GOOGLE_CLIENT_ID").unwrap_or_default();
    if client_id.is_empty() {
        return Redirect::temporary("/#get-started?oauth=configure").into_response();
    }
    let state_param = Uuid::new_v4().simple().to_string();
    state.platform.tenants.register_oauth_state(&state_param);
    let redirect_uri = format!("{}/api/v1/auth/oauth/google/callback", site_url(&headers));
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&state={}&access_type=online&prompt=select_account",
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&state_param),
    );
    Redirect::temporary(&url).into_response()
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn oauth_google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<OAuthCallback>,
) -> Response {
    if q.error.is_some() {
        return Redirect::temporary("/#get-started?oauth=denied").into_response();
    }
    let Some(code) = q.code.filter(|c| !c.is_empty()) else {
        return Redirect::temporary("/#get-started?oauth=missing_code").into_response();
    };
    let Some(st) = q.state.filter(|s| !s.is_empty()) else {
        return Redirect::temporary("/#get-started?oauth=missing_state").into_response();
    };
    if !state.platform.tenants.consume_oauth_state(&st) {
        return Redirect::temporary("/#get-started?oauth=bad_state").into_response();
    }

    let client_id = std::env::var("THINE_OAUTH_GOOGLE_CLIENT_ID").unwrap_or_default();
    let client_secret = std::env::var("THINE_OAUTH_GOOGLE_CLIENT_SECRET").unwrap_or_default();
    if client_id.is_empty() || client_secret.is_empty() {
        return Redirect::temporary("/#get-started?oauth=configure").into_response();
    }
    let redirect_uri = format!("{}/api/v1/auth/oauth/google/callback", site_url(&headers));

    let token_resp = match exchange_google_code(&client_id, &client_secret, &redirect_uri, &code).await
    {
        Ok(t) => t,
        Err(_) => return Redirect::temporary("/#get-started?oauth=token_error").into_response(),
    };
    let profile = match fetch_google_userinfo(&token_resp.access_token).await {
        Ok(p) => p,
        Err(_) => return Redirect::temporary("/#get-started?oauth=profile_error").into_response(),
    };
    let email = profile.email.unwrap_or_else(|| "user@unknown".into());
    let org_hint = profile.name.as_deref();
    let session = state
        .platform
        .issue_token_for_customer(&email, org_hint, vec!["admin".into()]);
    state
        .platform
        .tenants
        .advance_onboarding(&session.org_id, "welcome");
    Redirect::temporary(&format!(
        "/#get-started?token={}&org={}",
        urlencoding::encode(&session.token),
        urlencoding::encode(&session.org_id),
    ))
    .into_response()
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResp {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    name: Option<String>,
}

async fn exchange_google_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<GoogleTokenResp, reqwest::Error> {
    let client = reqwest::Client::new();
    let body = format!(
        "code={}&client_id={}&client_secret={}&redirect_uri={}&grant_type=authorization_code",
        urlencoding::encode(code),
        urlencoding::encode(client_id),
        urlencoding::encode(client_secret),
        urlencoding::encode(redirect_uri),
    );
    client
        .post("https://oauth2.googleapis.com/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

async fn fetch_google_userinfo(access_token: &str) -> Result<GoogleUserInfo, reqwest::Error> {
    let client = reqwest::Client::new();
    client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

#[derive(Debug, Serialize)]
pub struct OAuthConfig {
    pub google_enabled: bool,
    pub signup_enabled: bool,
    pub demo_org_id: String,
}

pub async fn oauth_config() -> Json<OAuthConfig> {
    Json(OAuthConfig {
        google_enabled: !std::env::var("THINE_OAUTH_GOOGLE_CLIENT_ID")
            .unwrap_or_default()
            .is_empty(),
        signup_enabled: true,
        demo_org_id: DEMO_ORG_ID.into(),
    })
}
