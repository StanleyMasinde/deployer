use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use deployer::http::frontend::static_frontend;
use tower::ServiceExt;

#[tokio::test]
async fn serves_ssg_spa_and_404_for_missing_assets() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("assets")).unwrap();
    std::fs::write(directory.path().join("index.html"), "public SSG page").unwrap();
    std::fs::write(directory.path().join("spa.html"), "application shell").unwrap();
    let app = static_frontend(Router::new(), directory.path()).unwrap();

    for (path, expected, status) in [
        ("/", "public SSG page", StatusCode::OK),
        ("/console", "application shell", StatusCode::OK),
        ("/assets/missing.js", "", StatusCode::NOT_FOUND),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status);

        if status == StatusCode::OK {
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            assert_eq!(bytes, expected);
        }
    }
}
