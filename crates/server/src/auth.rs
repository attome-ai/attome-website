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

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct GoogleTokenInfo {
    sub:   String,
    email: String,
    aud:   String,
}

#[derive(Deserialize)]
pub struct GoogleAuthRequest {
    pub credential: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email:    String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email:    String,
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
    let new_id        = Uuid::new_v4();

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core_systemuser (id, email, password_hash, createdon)
         VALUES ($1, $2, $3, now())
         RETURNING id",
    )
    .bind(new_id)
    .bind(&body.email)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db)
            if db.constraint() == Some("core_systemuser_email_unique") =>
        {
            AppError::Conflict("email already registered")
        }
        other => AppError::internal(other),
    })?;

    let token  = issue_token(user_id, &state)?;
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
        "SELECT id, password_hash
         FROM core_systemuser
         WHERE lower(email) = lower($1)
           AND is_active = true
           AND password_hash IS NOT NULL",
    )
    .bind(&body.email)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::internal)?;

    let (user_id, stored_hash) = row.ok_or(AppError::Unauthorized)?;

    if !verify_password(&body.password, &stored_hash)? {
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE core_systemuser SET last_login = now() WHERE id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(AppError::internal)?;

    let token  = issue_token(user_id, &state)?;
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

    // Upsert on email — links Google to existing password accounts automatically.
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core_systemuser (id, email, google_sub, createdon, last_login)
         VALUES (gen_random_uuid(), $1, $2, now(), now())
         ON CONFLICT (email) DO UPDATE
             SET google_sub = EXCLUDED.google_sub,
                 last_login = now()
         RETURNING id",
    )
    .bind(&info.email)
    .bind(&info.sub)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::internal)?;

    let token  = issue_token(user_id, &state)?;
    let cookie = set_cookie(&token, state.config.jwt_expiry_secs());

    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(AuthResponse { user_id }),
    )
        .into_response())
}

// ── Token creation ────────────────────────────────────────────────────────────

fn issue_token(user_id: Uuid, state: &AppState) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = SessionClaims {
        sub: user_id,
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
