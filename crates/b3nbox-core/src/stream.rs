use crate::backend::BackendError;
use futures_util::Stream;
use std::pin::Pin;

#[derive(Debug)]
pub enum StreamError {
    Http(reqwest::Error),
    Parse(String),
    Api(String),
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::Http(e) => write!(f, "HTTP-Fehler: {e}"),
            StreamError::Parse(m) => write!(f, "Parse-Fehler: {m}"),
            StreamError::Api(m) => write!(f, "API-Fehler: {m}"),
        }
    }
}

impl StreamError {
    pub fn wrap_backend(self, backend: String) -> BackendError {
        match self {
            StreamError::Http(e) => BackendError::Http(e),
            StreamError::Parse(m) => BackendError::Api(backend, format!("Stream-Parse-Fehler: {m}")),
            StreamError::Api(m) => BackendError::Api(backend, m),
        }
    }
}

pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<String, StreamError>> + Send>>;

/// Parst einen Byte-Stream (SSE `data:`-Zeilen oder JSON-Lines) und extrahiert
/// Text-Chunks über die `extract`-Funktion.
pub fn sse_chat_stream<S, F>(byte_stream: S, extract: F) -> ChunkStream
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    F: Fn(&serde_json::Value) -> Option<String> + Send + 'static,
{

    let mut buffer = String::new();
    let mut stream = Box::pin(byte_stream);

    let inner = futures_util::stream::poll_fn(move |cx| {
        loop {
            match stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buffer.find('\n') {
                        let line: String = buffer.drain(..=pos).collect();
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data.trim() == "[DONE]" {
                                return Poll::Ready(None);
                            }
                            match serde_json::from_str::<serde_json::Value>(data.trim()) {
                                Ok(value) => {
                                    if let Some(chunk) = extract(&value) {
                                        return Poll::Ready(Some(Ok(chunk)));
                                    }
                                }
                                Err(e) => {
                                    return Poll::Ready(Some(Err(StreamError::Parse(e.to_string()))));
                                }
                            }
                        } else if line.starts_with('{') {
                            match serde_json::from_str::<serde_json::Value>(line) {
                                Ok(value) => {
                                    if let Some(chunk) = extract(&value) {
                                        return Poll::Ready(Some(Ok(chunk)));
                                    }
                                }
                                Err(e) => {
                                    return Poll::Ready(Some(Err(StreamError::Parse(e.to_string()))));
                                }
                            }
                        }
                    }
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(StreamError::Http(e)))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    });

    Box::pin(inner)
}

use std::task::Poll;

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    async fn collect(stream: ChunkStream) -> Vec<Result<String, String>> {
        futures_util::StreamExt::collect::<Vec<_>>(stream)
            .await
            .into_iter()
            .map(|r| r.map_err(|e| e.to_string()))
            .collect()
    }

    #[tokio::test]
    async fn parses_openai_sse_lines() {
        let lines = vec![
            Ok(bytes::Bytes::from(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hal\"}}]}\n",
            )),
            Ok(bytes::Bytes::from(
                "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n",
            )),
            Ok(bytes::Bytes::from("data: [DONE]\n")),
        ];
        let byte_stream = stream::iter(lines);
        let s = sse_chat_stream(byte_stream, |v| {
            v["choices"][0]["delta"]["content"].as_str().map(String::from)
        });
        let chunks = collect(s).await;
        assert_eq!(
            chunks,
            vec![Ok("Hal".into()), Ok("lo".into())]
        );
    }

    #[tokio::test]
    async fn parses_json_lines() {
        let lines = vec![
            Ok(bytes::Bytes::from("{\"message\":{\"content\":\"A\"}}\n")),
            Ok(bytes::Bytes::from("{\"message\":{\"content\":\"B\"}}\n")),
        ];
        let byte_stream = stream::iter(lines);
        let s = sse_chat_stream(byte_stream, |v| {
            v["message"]["content"].as_str().map(String::from)
        });
        let chunks = collect(s).await;
        assert_eq!(chunks, vec![Ok("A".into()), Ok("B".into())]);
    }

    #[tokio::test]
    async fn chunks_split_across_reads() {
        let lines = vec![
            Ok(bytes::Bytes::from("data: {\"choices\":[{\"delta\":{\"con")),
            Ok(bytes::Bytes::from("tent\":\"X\"}}]}\ndata: [DONE]\n")),
        ];
        let byte_stream = stream::iter(lines);
        let s = sse_chat_stream(byte_stream, |v| {
            v["choices"][0]["delta"]["content"].as_str().map(String::from)
        });
        let chunks = collect(s).await;
        assert_eq!(chunks, vec![Ok("X".into())]);
    }
}
