use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use deployer::{application::sessions as auth, http::jsonapi};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
struct Input {
    name: String,
}

impl jsonapi::Input for Input {
    const TYPE: &'static str = "apps";
}

#[test]
fn decodes_create_documents_and_checks_resource_type() {
    let input: Input =
        jsonapi::decode(br#"{"data":{"type":"apps","attributes":{"name":"home"}}}"#).unwrap();
    assert_eq!(input.name, "home");

    let error =
        jsonapi::decode::<Input>(br#"{"data":{"type":"sessions","attributes":{"name":"home"}}}"#)
            .unwrap_err();
    assert_eq!(error.0, StatusCode::CONFLICT);
    assert!(jsonapi::decode::<Input>(br#"{"name":"home"}"#).is_err());
}

#[tokio::test]
async fn writes_real_jsonapi_resources_and_errors() {
    #[derive(Serialize)]
    struct App {
        id: u64,
        name: String,
    }

    let response = jsonapi::one(jsonapi::resource(
        "apps",
        1,
        App {
            id: 1,
            name: "home".into(),
        },
    ))
    .into_response();
    assert_eq!(response.headers()["content-type"], jsonapi::MEDIA_TYPE);

    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    let document: jsonapi_crate::api::JsonApiDocument = serde_json::from_slice(&bytes).unwrap();
    assert!(document.is_valid());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["data"],
        json!({"type":"apps","id":"1","attributes":{"name":"home"}})
    );

    let response = jsonapi::error(StatusCode::UNAUTHORIZED, "Sign in".into());
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["errors"][0]["status"],
        "401"
    );
}

#[test]
fn sessions_expire_and_reject_tampering() {
    let token = auth::issue("a secret with sufficient entropy", 1100);

    assert!(auth::valid(
        "a secret with sufficient entropy",
        &token,
        1000
    ));
    assert!(!auth::valid("different secret", &token, 1000));
    assert!(!auth::valid(
        "a secret with sufficient entropy",
        &token,
        1100
    ));
    assert!(!auth::valid(
        "a secret with sufficient entropy",
        "1100.fake",
        1000
    ));
}

extern crate jsonapi as jsonapi_crate;
