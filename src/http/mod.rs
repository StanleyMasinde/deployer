pub mod auth;
pub mod frontend;
mod handlers;
pub mod jsonapi;

use crate::{
    config::Config,
    providers::{CurrentProvider, current_provider},
    storage::Store,
};
use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct State {
    pub store: Store,
    pub config: Arc<Config>,
    pub current_provider: Arc<CurrentProvider>,
    pub setup: Arc<crate::application::setup::Setup>,
    pub login_slot: Arc<tokio::sync::Semaphore>,
}

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);
impl ApiError {
    fn internal(error: anyhow::Error) -> Self {
        tracing::error!(%error,"request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Operation failed. Check the Deployer service logs for details.".into(),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        jsonapi::error(self.0, self.1)
    }
}

pub fn router(state: State) -> Router {
    let protected = Router::new()
        .route(
            "/api/session",
            get(handlers::session).delete(handlers::logout),
        )
        .route("/api/apps", get(handlers::apps).post(handlers::add_app))
        .route(
            "/api/apps/{id}",
            get(handlers::app)
                .put(handlers::update_app)
                .delete(handlers::delete_app),
        )
        .route("/api/apps/{id}/deployments", get(handlers::app_deployments))
        .route("/api/apps/{id}/workflow", post(handlers::workflow))
        .route("/api/apps/{id}/https", post(handlers::enable_https))
        .route("/api/deployments", get(handlers::deployments))
        .route("/api/repositories", get(handlers::repositories))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::protect,
        ));
    protected
        .route(
            "/api/setup",
            get(handlers::setup_status).post(handlers::setup_root),
        )
        .route("/api/session", post(handlers::login))
        .route("/api/webhooks/github", post(handlers::webhook))
        .route(
            "/health",
            get(|| async { Json(serde_json::json!({"status":"ok","version":crate::VERSION})) }),
        )
        .route("/health/readiness", get(handlers::readiness))
        .route(
            "/api",
            axum::routing::any(|| async {
                ApiError(StatusCode::NOT_FOUND, "Endpoint not found".into())
            }),
        )
        .route(
            "/api/{*path}",
            axum::routing::any(|| async {
                ApiError(StatusCode::NOT_FOUND, "Endpoint not found".into())
            }),
        )
        .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(axum::middleware::map_response(
            |mut response: Response| async move {
                response
                    .headers_mut()
                    .insert("x-content-type-options", "nosniff".parse().unwrap());
                response
                    .headers_mut()
                    .insert("cache-control", "no-store".parse().unwrap());
                response
            },
        ))
        .with_state(state)
}

pub async fn run(
    config: Config,
    migrations: &toasty::migration::MigrationSet,
) -> anyhow::Result<()> {
    let store = Store::connect(&config.database.url).await?;
    // Apply embedded migrations before setup detection or accepting requests.
    migrations.apply(&store.database()).await?;
    let current_provider = current_provider(&config.providers)?;
    let setup =
        Arc::new(crate::application::setup::Setup::load(&store, &config.server.state_root).await?);
    let directory = config.frontend_directory();
    let address = config.server.listen;
    let state = State {
        store,
        current_provider,
        config: Arc::new(config),
        setup,
        login_slot: Arc::new(tokio::sync::Semaphore::new(1)),
    };
    let (app, _vite) = frontend::attach(router(state), &directory).await?;

    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address,version=crate::VERSION,"Deployer listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(crate::shutdown())
        .await?;

    Ok(())
}
