use super::ApiError;
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use jsonapi::api::{DocumentData, DocumentError, JsonApiError, JsonApiInfo, PrimaryData, Resource};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const MEDIA_TYPE: &str = "application/vnd.api+json";

pub trait Input: DeserializeOwned {
    const TYPE: &'static str;
}

pub struct ApiInput<T>(pub T);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputDocument {
    data: InputResource,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputResource {
    #[serde(rename = "type")]
    kind: String,
    attributes: serde_json::Value,
}

pub fn decode<T: Input>(body: &[u8]) -> Result<T, ApiError> {
    let document: InputDocument = serde_json::from_slice(body).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Expected a JSON:API resource document".into(),
        )
    })?;

    if document.data.kind != T::TYPE {
        return Err(ApiError(
            StatusCode::CONFLICT,
            format!("Expected resource type {}", T::TYPE),
        ));
    }

    serde_json::from_value(document.data.attributes).map_err(|_| {
        ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Invalid resource attributes".into(),
        )
    })
}

impl<S: Send + Sync, T: Input> FromRequest<S> for ApiInput<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        if request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            != Some(MEDIA_TYPE)
        {
            return Err(ApiError(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!("Use Content-Type: {MEDIA_TYPE}"),
            ));
        }

        let bytes = Bytes::from_request(request, state).await.map_err(|_| {
            ApiError(
                StatusCode::BAD_REQUEST,
                "Request body is invalid or too large".into(),
            )
        })?;

        decode(&bytes).map(Self)
    }
}

pub struct ApiDocument(pub DocumentData);

pub fn resource(kind: &str, id: impl ToString, attributes: impl Serialize) -> Resource {
    let value = serde_json::to_value(attributes).expect("resource attributes serialize");
    let mut attributes = value
        .as_object()
        .expect("resource attributes are an object")
        .clone();
    attributes.remove("id");

    Resource {
        _type: kind.into(),
        id: id.to_string(),
        attributes: attributes.into_iter().collect(),
        ..Resource::default()
    }
}

pub fn one(resource: Resource) -> ApiDocument {
    document(PrimaryData::Single(Box::new(resource)))
}

pub fn many(resources: Vec<Resource>) -> ApiDocument {
    document(PrimaryData::Multiple(resources))
}

fn document(data: PrimaryData) -> ApiDocument {
    ApiDocument(DocumentData {
        data: Some(data),
        jsonapi: Some(JsonApiInfo {
            version: Some("1.0".into()),
            meta: None,
        }),
        ..DocumentData::default()
    })
}

impl IntoResponse for ApiDocument {
    fn into_response(self) -> Response {
        (
            [(header::CONTENT_TYPE, MEDIA_TYPE)],
            serde_json::to_vec(&self.0).expect("document serializes"),
        )
            .into_response()
    }
}

pub fn error(status: StatusCode, detail: String) -> Response {
    let document = DocumentError {
        errors: vec![JsonApiError {
            status: Some(status.as_u16().to_string()),
            title: Some(status.canonical_reason().unwrap_or("Request failed").into()),
            detail: Some(detail),
            ..JsonApiError::default()
        }],
        ..DocumentError::default()
    };

    (
        status,
        [(header::CONTENT_TYPE, MEDIA_TYPE)],
        serde_json::to_vec(&document).expect("error serializes"),
    )
        .into_response()
}

impl Input for crate::domain::NewApp {
    const TYPE: &'static str = "apps";
}
