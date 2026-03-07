use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
// use tracing::info; (removed unused)
use crate::api::{LoginStatus, VRChatAPI};
use crate::db::{Database, PersonalityData};

pub struct AppState {
    pub db: Arc<Database>,
    pub api: Arc<VRChatAPI>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://127.0.0.1:3001".parse().unwrap(),
            "http://localhost:3001".parse().unwrap(),
            "http://127.0.0.1:5173".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/active", get(get_active_players))
        .route("/api/users", get(get_all_users))
        .route("/api/auth/status", get(get_auth_status))
        .route("/api/auth/login", post(post_login))
        .route("/api/auth/2fa", post(post_2fa))
        .route("/api/auth/logout", post(post_logout))
        .route("/api/personality", post(post_personality))
        .layer(cors)
        .with_state(state)
}

async fn get_active_players(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let players = state.db.get_active_players();
    Json(json!(players))
}

async fn get_all_users(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let users = state.db.get_all_users_with_personality();
    Json(json!(users))
}

async fn get_auth_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let result = state.api.check_auth().await;
    match result.status {
        LoginStatus::Success => Json(json!({
            "status": "logged_in",
            "user": result.user
        })),
        LoginStatus::TwoFactor => Json(json!({
            "status": "require_2fa",
            "requiresTwoFactorAuth": result.requires_two_factor_auth
        })),
        LoginStatus::Failed => Json(json!({
            "status": "logged_out"
        })),
    }
}

#[derive(serde::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn post_login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let result = state
        .api
        .login(Some(&body.username), Some(&body.password))
        .await;

    match result.status {
        LoginStatus::Success => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "status": "logged_in",
                "user": result.user
            })),
        ),
        LoginStatus::TwoFactor => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "status": "require_2fa",
                "requiresTwoFactorAuth": result.requires_two_factor_auth
            })),
        ),
        LoginStatus::Failed => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "message": result.message
            })),
        ),
    }
}

#[derive(serde::Deserialize)]
struct TwoFARequest {
    #[serde(rename = "type")]
    tfa_type: String,
    code: String,
}

async fn post_2fa(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TwoFARequest>,
) -> impl IntoResponse {
    let result = state.api.verify_2fa(&body.tfa_type, &body.code).await;

    match result.status {
        LoginStatus::Success => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "user": result.user
            })),
        ),
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "message": result.message
            })),
        ),
    }
}

async fn post_logout(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.api.logout().await;
    Json(json!({ "success": true }))
}

async fn post_personality(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PersonalityData>,
) -> impl IntoResponse {
    if body.user_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "userId is required" })),
        );
    }
    state.db.update_personality(&body);
    (StatusCode::OK, Json(json!({ "success": true })))
}
