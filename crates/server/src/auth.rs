use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use base_auth::{
    jwt::create_token,
    password::{hash_password, verify_password},
    session::SessionClaims,
};
use base_server::AppState;
use base_types::AppError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
struct GoogleTokenInfo {
    sub:   String,
    email: String,
    aud:   String,
}

// Phase 1: single system tenant; replaced with real tenant resolution in Phase 2.
const SYSTEM_TENANT: Uuid = Uuid::from_u128(1);

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GoogleAuthRequest {
    pub credential: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user_id: Uuid,
}

// ── Cookie helpers ────────────────────────────────────────────────────────────

fn set_cookie(token: &str, max_age_secs: u64) -> String {
    format!(
        "session_token={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age_secs}"
    )
}

fn clear_cookie() -> String {
    "session_token=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_owned()
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Response, AppError> {
    let password_hash = hash_password(&body.password)?;

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, email, password_hash)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(&body.email)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db) if db.constraint() == Some("uq_users_tenant_email") => {
            AppError::Conflict("email already registered")
        }
        other => AppError::internal(other),
    })?;

    let token = issue_token(user_id, SYSTEM_TENANT, &state)?;
    let cookie = set_cookie(&token, state.config.jwt_expiry_secs());

    Ok((
        StatusCode::CREATED,
        [(header::SET_COOKIE, cookie)],
        Json(AuthResponse { user_id }),
    )
        .into_response())
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, AppError> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, password_hash FROM users
         WHERE tenant_id = $1 AND email = $2 AND is_active = true",
    )
    .bind(SYSTEM_TENANT)
    .bind(&body.email)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::internal)?;

    let (user_id, stored_hash) = row.ok_or(AppError::Unauthorized)?;

    if !verify_password(&body.password, &stored_hash)? {
        return Err(AppError::Unauthorized);
    }

    let token = issue_token(user_id, SYSTEM_TENANT, &state)?;
    let cookie = set_cookie(&token, state.config.jwt_expiry_secs());

    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(AuthResponse { user_id }),
    )
        .into_response())
}

pub async fn logout() -> impl IntoResponse {
    (StatusCode::OK, [(header::SET_COOKIE, clear_cookie())])
}

pub async fn google_login(
    State(state): State<AppState>,
    Json(body): Json<GoogleAuthRequest>,
) -> Result<Response, AppError> {
    let url = format!(
        "https://oauth2.googleapis.com/tokeninfo?id_token={}",
        body.credential
    );
    let info: GoogleTokenInfo = reqwest::get(&url)
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(|_| AppError::Unauthorized)?
        .json()
        .await
        .map_err(AppError::internal)?;

    if info.aud != state.config.auth.oauth_google_client_id {
        return Err(AppError::Unauthorized);
    }

    let user_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (tenant_id, email, google_sub)
           VALUES ($1, $2, $3)
           ON CONFLICT (tenant_id, email)
           DO UPDATE SET google_sub = EXCLUDED.google_sub, updated_at = now()
           RETURNING id"#,
    )
    .bind(SYSTEM_TENANT)
    .bind(&info.email)
    .bind(&info.sub)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::internal)?;

    let token  = issue_token(user_id, SYSTEM_TENANT, &state)?;
    let cookie = set_cookie(&token, state.config.jwt_expiry_secs());

    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(AuthResponse { user_id }),
    )
        .into_response())
}

// ── Token creation ────────────────────────────────────────────────────────────

fn issue_token(user_id: Uuid, tenant_id: Uuid, state: &AppState) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = SessionClaims {
        sub: user_id,
        tenant_id,
        iat: now,
        exp: now + state.config.jwt_expiry_secs() as i64,
    };
    create_token(&claims, &state.config.auth.jwt_secret)
}

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/google",   post(google_login))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login",    post(login))
        .route("/api/v1/auth/logout",   post(logout))
}
