use super::{ApiError, State};
use axum::{
    extract::State as ExtractState,
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Sse,
        sse::{Event, KeepAlive},
    },
};
use std::{convert::Infallible, time::Duration};
use tokio_stream::wrappers::ReceiverStream;

pub async fn stream(
    ExtractState(state): ExtractState<State>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let token = headers
        .get("cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .filter_map(|c| c.trim().split_once('='))
                .find(|(key, _)| *key == "deployer_session")
                .map(|(_, value)| value.to_owned())
        })
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "Sign in to continue".into()))?;
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(2);
    tokio::spawn(async move {
        let mut previous = String::new();
        loop {
            let changed = state.store.changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if !state
                .store
                .valid_session(&state.config.server.session_secret, &token, crate::now())
                .await
                .unwrap_or(false)
            {
                break;
            }
            let snapshot = match tokio::try_join!(state.store.apps(), state.store.deployments()) {
                Ok((apps, deployments)) => serde_json::to_string(
                    &serde_json::json!({"apps": apps, "deployments": deployments}),
                ),
                Err(error) => {
                    tracing::warn!(%error, "could not refresh event stream");
                    break;
                }
            };
            let Ok(snapshot) = snapshot else {
                break;
            };
            if snapshot != previous {
                let event = Event::default().event("snapshot").data(snapshot.clone());
                if sender.send(Ok(event)).await.is_err() {
                    break;
                }
                previous = snapshot;
            }
            // Webhooks wake the stream immediately. The bounded server-side check
            // also observes updates made by the separate worker process.
            tokio::select! {
                _ = sender.closed() => break,
                _ = changed => {},
                _ = tokio::time::sleep(Duration::from_secs(2)) => {},
            }
        }
    });
    Ok((
        [("x-accel-buffering", "no")],
        Sse::new(ReceiverStream::new(receiver))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    ))
}
