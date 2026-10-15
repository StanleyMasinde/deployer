use anyhow::{Context, ensure};
use axum::{Router, body::Body, extract::Request, response::Response};
use std::{path::Path, process::Child};
use tower_http::services::{ServeDir, ServeFile};

pub struct Vite(Option<Child>);
impl Drop for Vite {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub async fn attach(app: Router, directory: &Path) -> anyhow::Result<(Router, Vite)> {
    if cfg!(debug_assertions) {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        let mut vite = Vite(None);
        if client
            .get("http://127.0.0.1:5173/@vite/client")
            .send()
            .await
            .is_err()
        {
            vite.0 = Some(
                std::process::Command::new("node_modules/.bin/vite")
                    .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend"))
                    .spawn()
                    .context("start Vite; install frontend dependencies first")?,
            );
        }

        for _ in 0..100 {
            if client
                .get("http://127.0.0.1:5173/@vite/client")
                .send()
                .await
                .is_ok()
            {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        let app = app.fallback(move |request: Request| {
            let client = client.clone();
            async move {
                let path = request
                    .uri()
                    .path_and_query()
                    .map(|p| p.as_str())
                    .unwrap_or("/");
                let result = client
                    .request(
                        request.method().clone(),
                        format!("http://127.0.0.1:5173{path}"),
                    )
                    .send()
                    .await;
                match result {
                    Ok(response) => {
                        let mut builder = Response::builder().status(response.status());
                        for (key, value) in response.headers() {
                            if !matches!(
                                key.as_str(),
                                "transfer-encoding" | "connection" | "content-length"
                            ) {
                                builder = builder.header(key, value);
                            }
                        }
                        builder
                            .body(Body::from(response.bytes().await.unwrap_or_default()))
                            .expect("proxy response")
                    }
                    Err(_) => Response::builder()
                        .status(502)
                        .body(Body::from("Vite is unavailable"))
                        .expect("proxy error"),
                }
            }
        });
        return Ok((app, vite));
    }

    Ok((static_frontend(app, directory)?, Vite(None)))
}

pub fn static_frontend(app: Router, directory: &Path) -> anyhow::Result<Router> {
    ensure!(
        directory.join("index.html").is_file() && directory.join("spa.html").is_file(),
        "frontend for version {} is missing at {}",
        crate::VERSION,
        directory.display()
    );
    // Missing JS/CSS must be 404, never the SPA shell.
    let app = app
        .nest_service("/assets", ServeDir::new(directory.join("assets")))
        .fallback_service(
            ServeDir::new(directory).fallback(ServeFile::new(directory.join("spa.html"))),
        );
    Ok(app)
}
