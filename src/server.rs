use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
// use tracing::info;
use crate::api::{LoginStatus, VRChatAPI};
use crate::bio::BioManager;
use crate::db::Database;

pub struct AppState {
    pub db: Arc<Database>,
    pub api: Arc<VRChatAPI>,
    pub bio: Arc<BioManager>,
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
        .route("/api/vrc/user/:id", get(get_vrc_user_info))
        .layer(cors)
        .with_state(state)
}

async fn get_active_players(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let players = state.db.get_active_players();
    Json(json!(players))
}

async fn get_all_users(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let users = state.db.get_all_users();
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

// Removed post_personality

async fn get_vrc_user_info(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    match state.bio.process_user(&user_id, true, None).await {
        Ok(user_data) => {
            axum::response::Response::builder()
                .header("content-type", "application/json; charset=utf-8")
                .body(axum::body::Body::from(serde_json::to_string(&user_data).unwrap()))
                .unwrap()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// Redundant formatter removed (now in bio.rs)
