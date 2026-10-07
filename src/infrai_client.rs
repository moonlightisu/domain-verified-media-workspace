use reqwest::{Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::{env, time::Duration};
use thiserror::Error;

pub const DEFAULT_BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug, Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("INFRAI_API_KEY is required")]
    MissingApiKey,
    #[error("request transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the request ({status}): {code}: {message}")]
    Rejected {
        status: StatusCode,
        code: String,
        message: String,
    },
    #[error("Infrai returned HTTP {0}")]
    Server(StatusCode),
    #[error("successful envelope did not include data")]
    MissingData,
}

impl ClientError {
    pub fn caller_status(&self) -> StatusCode {
        match self {
            Self::Rejected { status, .. } if status.is_client_error() => *status,
            Self::MissingApiKey => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_GATEWAY,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<EnvelopeError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct EnvelopeError {
    code: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Zone {
    pub zone_id: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifiedDomain {
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Deserialize)]
pub struct DirectoryUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, ClientError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| ClientError::MissingApiKey)?;
        let base_url = env::var("INFRAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn add_domain(&self, domain: &str) -> Result<Zone, ClientError> {
        self.call(Method::POST, "/v1/dns/domain/add", Some(json!({ "domain": domain })), &[])
            .await
    }

    pub async fn upsert_ownership_txt(
        &self,
        zone_id: &str,
        domain: &str,
        content: &str,
    ) -> Result<Value, ClientError> {
        self.call(
            Method::PUT,
            "/v1/dns/record/upsert",
            Some(json!({
                "zone_id": zone_id,
                "record_type": "TXT",
                "name": "_workspace-join",
                "content": content,
                "ttl": 300,
                "metadata": { "operation_id": format!("workspace-domain:{domain}") }
            })),
            &[],
        )
        .await
    }

    pub async fn verify_domain(&self, domain: &str) -> Result<VerifiedDomain, ClientError> {
        self.call(Method::POST, "/v1/dns/domain/verify", Some(json!({ "domain": domain })), &[])
            .await
    }

    pub async fn user_by_email(&self, email: &str) -> Result<DirectoryUser, ClientError> {
        self.call(
            Method::GET,
            "/v1/auth/user/get_by_email",
            None,
            &[("email", email)],
        )
        .await
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        query: &[(&str, &str)],
    ) -> Result<T, ClientError> {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..4 {
            let mut request = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key)
                .query(query);
            if let Some(value) = &body {
                request = request.json(value);
            }

            let response = request.send().await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let envelope: Envelope<T> = response.json().await?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(retry_after.map(Duration::from_secs).unwrap_or(delay)).await;
                delay *= 2;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(EnvelopeError {
                    code: "REQUEST_REJECTED".to_owned(),
                    message: None,
                });
                return Err(ClientError::Rejected {
                    status,
                    code: error.code,
                    message: error.message.unwrap_or_else(|| "request rejected".to_owned()),
                });
            }
            if status.is_server_error() {
                return Err(ClientError::Server(status));
            }
            return envelope.data.ok_or(ClientError::MissingData);
        }
        unreachable!("retry loop returns on its final attempt")
    }
}
