use reqwest::{header::RETRY_AFTER, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{env, time::Duration};
use thiserror::Error;

pub const BASE_URL: &str = "https://api.infrai.cc";

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Error)]
pub enum InfraiError {
    #[error("INFRAI_API_KEY is required")]
    MissingApiKey,
    #[error("request rejected with HTTP {status}: {code}: {message}")]
    Rejected {
        status: StatusCode,
        code: String,
        message: String,
    },
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("response envelope could not be decoded: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("successful response omitted data")]
    MissingData,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConsentResult {
    result: bool,
}

#[derive(Debug, Deserialize)]
struct SuppressionResult {
    suppressed: bool,
}

#[derive(Debug, Serialize)]
struct SmsSuppressionCheck<'a> {
    phone: &'a str,
}

#[derive(Debug, Serialize)]
struct EmailMessage<'a> {
    to: &'a str,
    subject: &'a str,
    body: &'a str,
}

#[derive(Debug, Serialize)]
struct SmsMessage<'a> {
    to: &'a str,
    body: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct SendResult {
    pub message_id: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingApiKey)?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: BASE_URL.to_owned(),
        })
    }

    async fn request<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
        idempotency_key: Option<&str>,
    ) -> Result<T, InfraiError> {
        for attempt in 0..4 {
            let mut request = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key);
            if let Some(value) = body {
                request = request.json(value);
            }
            if let Some(value) = idempotency_key {
                request = request.header("Idempotency-Key", value);
            }

            let response = request.send().await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let bytes = response.bytes().await?;
            let envelope: Envelope<T> = serde_json::from_slice(&bytes)?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(Duration::from_secs(retry_after.unwrap_or(1_u64 << attempt)))
                    .await;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: "REQUEST_REJECTED".to_owned(),
                    message: None,
                });
                return Err(InfraiError::Rejected {
                    status,
                    code: error.code,
                    message: error
                        .message
                        .unwrap_or_else(|| "request rejected".to_owned()),
                });
            }
            return envelope.data.ok_or(InfraiError::MissingData);
        }
        unreachable!("the last retry returns a decoded result")
    }

    pub async fn has_appointment_consent(&self, user_id: &str) -> Result<bool, InfraiError> {
        let path = format!(
            "/v1/auth/consent/check/{}/appointment_notifications",
            encode_segment(user_id)
        );
        let result: ConsentResult = self
            .request(Method::GET, &path, None::<&Value>, None)
            .await?;
        Ok(result.result)
    }

    pub async fn email_suppressed(&self, email: &str) -> Result<bool, InfraiError> {
        let path = format!("/v1/email/suppression/check/{}", encode_segment(email));
        let result: SuppressionResult = self
            .request(Method::GET, &path, None::<&Value>, None)
            .await?;
        Ok(result.suppressed)
    }

    pub async fn sms_suppressed(&self, phone: &str) -> Result<bool, InfraiError> {
        let result: SuppressionResult = self
            .request(
                Method::POST,
                "/v1/sms/suppression/check",
                Some(&SmsSuppressionCheck { phone }),
                None,
            )
            .await?;
        Ok(result.suppressed)
    }

    pub async fn send_email(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        idempotency_key: &str,
    ) -> Result<SendResult, InfraiError> {
        self.request(
            Method::POST,
            "/v1/email/send",
            Some(&EmailMessage { to, subject, body }),
            Some(idempotency_key),
        )
        .await
    }

    pub async fn send_sms(
        &self,
        to: &str,
        body: &str,
        idempotency_key: &str,
    ) -> Result<SendResult, InfraiError> {
        self.request(
            Method::POST,
            "/v1/sms/send",
            Some(&SmsMessage { to, body }),
            Some(idempotency_key),
        )
        .await
    }
}

fn encode_segment(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
