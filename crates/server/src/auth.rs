use axum::{
    extract::{Extension, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use base_auth::{
    jwt::create_token,
    password::{hash_password, verify_password},
    session::SessionClaims,
};
use base_server::AppState;
use base_types::AppError;
use chrono::{DateTime, Utc};
use rand::distributions::{Alphanumeric, DistString};
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

// ── API key management ────────────────────────────────────────────────────────

fn generate_api_key(prefix: &str) -> String {
    let random = Alphanumeric.sample_string(&mut rand::thread_rng(), 40);
    format!("{prefix}_{random}")
}

#[derive(Serialize)]
pub struct ApiKeyMeta {
    pub id:           Uuid,
    pub name:         String,
    pub public_key:   String,
    pub created_at:   DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_active:    bool,
}

#[derive(Serialize)]
pub struct ApiKeyCreated {
    pub id:         Uuid,
    pub name:       String,
    pub public_key: String,
    pub secret:     String, // returned only on creation — never again
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateApiKeyBody {
    pub name: String,
}

pub async fn list_api_keys(
    State(state):    State<AppState>,
    Extension(claims): Extension<SessionClaims>,
) -> Result<Json<Vec<ApiKeyMeta>>, AppError> {
    let rows: Vec<(Uuid, String, String, DateTime<Utc>, Option<DateTime<Utc>>, bool)> =
        sqlx::query_as(
            "SELECT id, name, public_key, created_at, last_used_at, is_active
             FROM core_apikey
             WHERE user_ref = $1
             ORDER BY created_at DESC",
        )
        .bind(claims.sub)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::internal)?;

    let keys = rows
        .into_iter()
        .map(|(id, name, public_key, created_at, last_used_at, is_active)| ApiKeyMeta {
            id,
            name,
            public_key,
            created_at,
            last_used_at,
            is_active,
        })
        .collect();

    Ok(Json(keys))
}

pub async fn create_api_key(
    State(state):      State<AppState>,
    Extension(claims): Extension<SessionClaims>,
    Json(body):        Json<CreateApiKeyBody>,
) -> Result<(StatusCode, Json<ApiKeyCreated>), AppError> {
    let name = body.name.trim().to_owned();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }

    let public_key  = generate_api_key("atk_pub");
    let secret      = generate_api_key("atk_sec");
    let secret_hash = hash_password(&secret)?;

    let (id, created_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO core_apikey (user_ref, name, public_key, secret_hash)
         VALUES ($1, $2, $3, $4)
         RETURNING id, created_at",
    )
    .bind(claims.sub)
    .bind(&name)
    .bind(&public_key)
    .bind(&secret_hash)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::internal)?;

    Ok((
        StatusCode::CREATED,
        Json(ApiKeyCreated { id, name, public_key, secret, created_at }),
    ))
}

pub async fn revoke_api_key(
    State(state):      State<AppState>,
    Extension(claims): Extension<SessionClaims>,
    Path(key_id):      Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let n = sqlx::query(
        "UPDATE core_apikey SET is_active = false
         WHERE id = $1 AND user_ref = $2 AND is_active = true",
    )
    .bind(key_id)
    .bind(claims.sub)
    .execute(&state.db)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

// ── Routes ────────────────────────────────────────────────────────────────────

/// Public auth routes — no session required.
pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/google",   post(google_login))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login",    post(login))
        .route("/api/v1/auth/logout",   post(logout))
}

/// Protected auth routes — require a valid session (authn middleware applied by caller).
pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/api-keys",       get(list_api_keys).post(create_api_key))
        .route("/api/v1/auth/api-keys/{id}",  delete(revoke_api_key))
}
