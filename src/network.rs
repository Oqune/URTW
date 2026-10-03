use crate::model::DiagStatus;
use anyhow::Result;
use reqwest::{Client, Proxy};
use std::time::{Duration, Instant};

pub struct NetworkTester {
    client: Client,
}
impl NetworkTester {
    pub fn new(proxy_port: Option<u16>) -> Result<Self> {
        let mut builder = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(8))
            .user_agent(concat!("URTW/", env!("CARGO_PKG_VERSION")));
        if let Some(port) = proxy_port {
            builder = builder.proxy(Proxy::all(format!("http://127.0.0.1:{port}"))?);
        }
        Ok(Self {
            client: builder.build()?,
        })
    }
    pub async fn test(&self, url: &str) -> DiagStatus {
        let start = Instant::now();
        match self.client.get(url).send().await {
            Ok(response) => DiagStatus::Http {
                code: response.status().as_u16(),
                ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => DiagStatus::Failed(
                if e.is_timeout() {
                    "Timeout"
                } else if e.is_connect() {
                    "Connection / TLS error"
                } else if e.is_redirect() {
                    "Redirect error"
                } else {
                    "Request failed"
                }
                .into(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn reports_http_denial_without_calling_it_success() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0; 4096];
            let _ = stream.read(&mut buf).await;
            stream
                .write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
        });
        let status = NetworkTester::new(None)
            .unwrap()
            .test(&format!("http://127.0.0.1:{port}"))
            .await;
        assert!(status.finished());
        assert!(!status.successful());
        assert!(matches!(status, DiagStatus::Http { code: 403, .. }));
    }
}
