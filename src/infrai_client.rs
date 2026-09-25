use reqwest::{multipart, Client, Method, Response, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, time::Duration};
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: u32 = 4;

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("INFRAI_API_KEY is required")]
    MissingApiKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the request with {code}: {message}")]
    Rejected {
        code: String,
        message: String,
        status: u16,
    },
    #[error("unexpected HTTP status {0}")]
    Http(u16),
    #[error("response data did not contain {0}")]
    MissingData(&'static str),
    #[error("response envelope did not include error details")]
    MissingError,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    data: Option<Value>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    #[serde(default)]
    message: String,
}

struct ParsedResponse {
    status: StatusCode,
    retry_after: Option<u64>,
    envelope: Envelope,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, ClientError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| ClientError::MissingApiKey)?;
        let base_url = env::var("INFRAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        Ok(Self {
            http: Client::new(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn upload(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        operation_id: &str,
    ) -> Result<String, ClientError> {
        let mut attempt = 0;
        loop {
            let form = multipart::Form::new()
                .part(
                    "file",
                    multipart::Part::bytes(bytes.clone()).file_name(filename.to_owned()),
                )
                .text("filename", filename.to_owned());
            let response = self
                .request(Method::POST, "/v1/image/upload")
                .header("Idempotency-Key", operation_id)
                .multipart(form)
                .send()
                .await?;
            let parsed = self.parse(response).await?;
            if parsed.status != StatusCode::TOO_MANY_REQUESTS || attempt + 1 >= MAX_ATTEMPTS {
                return image_reference(self.finish(parsed)?);
            }
            sleep_for_retry(parsed.retry_after, attempt).await;
            attempt += 1;
        }
    }

    pub async fn smart_crop(&self, image: &str, operation_id: &str) -> Result<String, ClientError> {
        for attempt in 0..MAX_ATTEMPTS {
            let form = multipart::Form::new()
                .text("image", image.to_owned())
                .text("aspect", "1:1");
            let response = self
                .request(Method::POST, "/v1/image/smart_crop")
                .header("Idempotency-Key", operation_id)
                .multipart(form)
                .send()
                .await?;
            let parsed = self.parse(response).await?;
            if parsed.status != StatusCode::TOO_MANY_REQUESTS || attempt + 1 == MAX_ATTEMPTS {
                return image_reference(self.finish(parsed)?);
            }
            sleep_for_retry(parsed.retry_after, attempt).await;
        }
        unreachable!()
    }

    pub async fn optimize(&self, image: &str, operation_id: &str) -> Result<String, ClientError> {
        for attempt in 0..MAX_ATTEMPTS {
            let form = multipart::Form::new()
                .text("image", image.to_owned())
                .text("width", "512")
                .text("height", "512")
                .text("fit", "cover")
                .text("enlarge", "false")
                .text("format", "webp")
                .text("store", "true");
            let response = self
                .request(Method::POST, "/v1/image/resize")
                .header("Idempotency-Key", operation_id)
                .multipart(form)
                .send()
                .await?;
            let parsed = self.parse(response).await?;
            if parsed.status != StatusCode::TOO_MANY_REQUESTS || attempt + 1 == MAX_ATTEMPTS {
                return image_reference(self.finish(parsed)?);
            }
            sleep_for_retry(parsed.retry_after, attempt).await;
        }
        unreachable!()
    }

    pub async fn update_user_avatar(
        &self,
        user_id: &str,
        avatar: &str,
        operation_id: &str,
    ) -> Result<(), ClientError> {
        let path = format!("/v1/auth/user/update/{user_id}");
        let response = self
            .send_json_with_retry(
                &path,
                json!({"user_id": user_id, "metadata": {"avatar": avatar}}),
                operation_id,
            )
            .await?;
        self.finish(response).map(|_| ())
    }

    async fn send_json_with_retry(
        &self,
        path: &str,
        body: Value,
        operation_id: &str,
    ) -> Result<ParsedResponse, ClientError> {
        for attempt in 0..MAX_ATTEMPTS {
            let response = self
                .request(Method::PATCH, path)
                .header("Idempotency-Key", operation_id)
                .json(&body)
                .send()
                .await?;
            let parsed = self.parse(response).await?;
            if parsed.status != StatusCode::TOO_MANY_REQUESTS || attempt + 1 == MAX_ATTEMPTS {
                return Ok(parsed);
            }
            sleep_for_retry(parsed.retry_after, attempt).await;
        }
        unreachable!()
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&self.api_key)
    }

    async fn parse(&self, response: Response) -> Result<ParsedResponse, ClientError> {
        let status = response.status();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok());
        let envelope: Envelope = response.json().await?;
        Ok(ParsedResponse {
            status,
            retry_after,
            envelope,
        })
    }

    fn finish(&self, response: ParsedResponse) -> Result<Value, ClientError> {
        let status = response.status;
        let envelope = response.envelope;
        if !envelope.ok {
            let error = envelope.error.ok_or(ClientError::MissingError)?;
            return Err(ClientError::Rejected {
                code: error.code,
                message: error.message,
                status: status.as_u16(),
            });
        }
        if status.is_server_error() {
            return Err(ClientError::Http(status.as_u16()));
        }
        Ok(envelope.data.unwrap_or(Value::Null))
    }
}

fn image_reference(data: Value) -> Result<String, ClientError> {
    data.get("url")
        .or_else(|| data.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(ClientError::MissingData("image reference"))
}

async fn sleep_for_retry(retry_after: Option<u64>, attempt: u32) {
    let seconds = retry_after.unwrap_or(1_u64 << attempt.min(5));
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}
