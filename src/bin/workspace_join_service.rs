use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use domain_verified_media_workspace::{
    infrai_client::InfraiClient,
    workspace_join::{join_verified_employee, JoinError, JoinRequest},
};
use serde_json::json;
use std::{env, sync::Arc};

#[tokio::main]
async fn main() {
    let client = InfraiClient::from_env().expect("INFRAI_API_KEY must be set");
    let state = Arc::new(client);
    let app = Router::new()
        .route("/workspace/join", post(join_workspace))
        .with_state(state);
    let address = env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("listen address must be available");
    println!("workspace join service listening on {address}");
    axum::serve(listener, app).await.expect("HTTP server stopped");
}

async fn join_workspace(
    State(client): State<Arc<InfraiClient>>,
    Json(request): Json<JoinRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let outcome = join_verified_employee(&client, request).await?;
    Ok((StatusCode::OK, Json(outcome)))
}

struct ApiError(JoinError);

impl From<JoinError> for ApiError {
    fn from(error: JoinError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            JoinError::DomainMismatch { .. } | JoinError::DomainNotVerified => StatusCode::UNPROCESSABLE_ENTITY,
            JoinError::Infrai(error) => error.caller_status(),
        };
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}

