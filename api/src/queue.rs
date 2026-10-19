//! A minimal client for the Vercel Queues HTTP API.
//! See <https://vercel.com/docs/queues/api>.

use std::time::Duration;

use bytes::Bytes;
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap};
use serde::Deserialize;
use url::Url;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ERROR_DETAIL_BYTES: usize = 1024;

#[derive(Debug, Clone)]
pub struct QueueClient {
    base_url: Url,
    http: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: String,
    pub receipt_handle: String,
    pub delivery_count: u32,
    pub body: Bytes,
}

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("invalid queue URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("queue request: {0}")]
    Request(#[from] reqwest::Error),
    #[error("queue {operation}: {status}: {detail}")]
    Status {
        operation: &'static str,
        status: StatusCode,
        detail: String,
    },
    #[error("read received messages: {0}")]
    Multipart(#[from] multer::Error),
    #[error("receive response has no multipart boundary")]
    MissingBoundary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendResponse {
    message_id: String,
}

impl QueueClient {
    /// Talks to the queue in one region. Messages can only be received from the region
    /// they were sent to.
    pub fn new(region: &str) -> Result<Self, QueueError> {
        let base_url = Url::parse(&format!("https://{region}.vercel-queue.com/api/v3"))?;
        Self::with_base_url(base_url)
    }

    /// Talks to the Queues API at another address, such as a test server.
    pub fn with_base_url(base_url: Url) -> Result<Self, QueueError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self { base_url, http })
    }

    /// Publishes a JSON message. Returns `None` when Vercel accepted the message but
    /// deferred delivery during a regional failover, so there is no message ID yet.
    pub async fn send(
        &self,
        token: &str,
        topic: &str,
        body: Vec<u8>,
    ) -> Result<Option<String>, QueueError> {
        let response = self
            .http
            .post(self.url(&["topic", topic]))
            .bearer_auth(token)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await?;

        match response.status() {
            StatusCode::CREATED => {
                let payload: SendResponse = response.json().await?;
                Ok(Some(payload.message_id))
            }
            StatusCode::ACCEPTED => Ok(None),
            _ => Err(status_error("send", response).await),
        }
    }

    /// Leases up to `max_messages` messages for the consumer group. It returns right away,
    /// with no messages when the topic is empty.
    pub async fn receive(
        &self,
        token: &str,
        topic: &str,
        consumer: &str,
        max_messages: u32,
        visibility_timeout: Duration,
    ) -> Result<Vec<Message>, QueueError> {
        let response = self
            .http
            .post(self.url(&["topic", topic, "consumer", consumer]))
            .bearer_auth(token)
            .header(ACCEPT, "multipart/mixed")
            .header("Vqs-Max-Messages", max_messages)
            .header(
                "Vqs-Visibility-Timeout-Seconds",
                visibility_timeout.as_secs(),
            )
            .send()
            .await?;

        match response.status() {
            StatusCode::NO_CONTENT => Ok(Vec::new()),
            StatusCode::OK => read_messages(response).await,
            _ => Err(status_error("receive", response).await),
        }
    }

    /// Removes a processed message from the consumer group.
    pub async fn ack(
        &self,
        token: &str,
        topic: &str,
        consumer: &str,
        receipt_handle: &str,
    ) -> Result<(), QueueError> {
        let url = self.url(&[
            "topic",
            topic,
            "consumer",
            consumer,
            "lease",
            receipt_handle,
        ]);
        let response = self.http.delete(url).bearer_auth(token).send().await?;

        if response.status() != StatusCode::NO_CONTENT {
            return Err(status_error("ack", response).await);
        }
        Ok(())
    }

    /// Appends percent-encoded path segments, so a topic or receipt handle containing `/`
    /// stays one segment.
    fn url(&self, segments: &[&str]) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("the queue base URL is an http(s) URL")
            .pop_if_empty()
            .extend(segments);
        url
    }
}

/// Parses a multipart/mixed response, where each part is one message and the `Vqs-*` part
/// headers carry its metadata.
async fn read_messages(response: reqwest::Response) -> Result<Vec<Message>, QueueError> {
    let boundary = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<mime::Mime>().ok())
        .and_then(|content_type| {
            content_type
                .get_param(mime::BOUNDARY)
                .map(|boundary| boundary.to_string())
        })
        .ok_or(QueueError::MissingBoundary)?;

    let mut multipart = multer::Multipart::new(response.bytes_stream(), boundary);
    let mut messages = Vec::new();
    while let Some(part) = multipart.next_field().await? {
        let headers = part.headers();
        let id = part_header(headers, "Vqs-Message-Id");
        let receipt_handle = part_header(headers, "Vqs-Receipt-Handle");
        let delivery_count = part_header(headers, "Vqs-Delivery-Count")
            .parse()
            .unwrap_or(0);
        let body = part.bytes().await?;

        messages.push(Message {
            id,
            receipt_handle,
            delivery_count,
            body,
        });
    }
    Ok(messages)
}

fn part_header(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

async fn status_error(operation: &'static str, response: reqwest::Response) -> QueueError {
    let status = response.status();
    let body = response.bytes().await.unwrap_or_default();
    let detail = &body[..body.len().min(MAX_ERROR_DETAIL_BYTES)];
    QueueError::Status {
        operation,
        status,
        detail: String::from_utf8_lossy(detail).trim().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const BOUNDARY: &str = "vqs-boundary-test";

    fn client_for(server: &MockServer) -> QueueClient {
        QueueClient::with_base_url(server.uri().parse().unwrap()).unwrap()
    }

    #[tokio::test]
    async fn send_receive_ack() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/topic/checks"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string(r#"{"monitorId":1}"#))
            .respond_with(ResponseTemplate::new(201).set_body_string(r#"{"messageId":"msg_1"}"#))
            .expect(1)
            .mount(&server)
            .await;

        let multipart_body = format!(
            "--{BOUNDARY}\r\n\
             Content-Type: application/json\r\n\
             Vqs-Message-Id: msg_1\r\n\
             Vqs-Receipt-Handle: rh/1+a\r\n\
             Vqs-Delivery-Count: 2\r\n\r\n\
             {{\"monitorId\":1}}\r\n\
             --{BOUNDARY}--\r\n"
        );
        Mock::given(method("POST"))
            .and(path("/topic/checks/consumer/worker"))
            .and(header("vqs-max-messages", "10"))
            .and(header("vqs-visibility-timeout-seconds", "60"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                multipart_body,
                &format!("multipart/mixed; boundary={BOUNDARY}"),
            ))
            .expect(1)
            .mount(&server)
            .await;

        // The receipt handle's "/" must be escaped so the handle stays one path segment.
        Mock::given(method("DELETE"))
            .and(path("/topic/checks/consumer/worker/lease/rh%2F1+a"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let client = client_for(&server);

        let id = client
            .send("test-token", "checks", br#"{"monitorId":1}"#.to_vec())
            .await
            .unwrap();
        assert_eq!(id.as_deref(), Some("msg_1"));

        let messages = client
            .receive(
                "test-token",
                "checks",
                "worker",
                10,
                Duration::from_secs(60),
            )
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.id, "msg_1");
        assert_eq!(message.receipt_handle, "rh/1+a");
        assert_eq!(message.delivery_count, 2);
        assert_eq!(&message.body[..], br#"{"monitorId":1}"#);

        client
            .ack("test-token", "checks", "worker", &message.receipt_handle)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn receive_empty_topic() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let messages = client_for(&server)
            .receive(
                "test-token",
                "checks",
                "worker",
                10,
                Duration::from_secs(60),
            )
            .await
            .unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn send_deferred_during_failover() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;

        let id = client_for(&server)
            .send("test-token", "checks", b"{}".to_vec())
            .await
            .unwrap();
        assert_eq!(id, None);
    }

    #[tokio::test]
    async fn error_status_includes_response_detail() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string("token expired\n"))
            .mount(&server)
            .await;

        let error = client_for(&server)
            .send("test-token", "checks", b"{}".to_vec())
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "queue send: 401 Unauthorized: token expired"
        );
    }
}
