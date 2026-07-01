use crate::TranscriptionMessage;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, tungstenite::client::IntoClientRequest,
};

const XAI_STT_WS_URL: &str = "wss://api.x.ai/v1/stt";

#[derive(Debug, Deserialize)]
struct WsEvent {
    #[serde(rename = "type")]
    event_type: String,
    text: Option<String>,
    message: Option<String>,
}

#[derive(Debug)]
pub enum SttError {
    NetworkError(String),
    ApiError(String),
    ParseError(String),
}

impl std::fmt::Display for SttError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SttError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            SttError::ApiError(msg) => write!(f, "API error: {}", msg),
            SttError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for SttError {}

pub struct SpeechToTextClient {
    api_key: String,
}

impl SpeechToTextClient {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub async fn stream_transcribe(
        &self,
        sample_rate: u32,
        mut audio_rx: mpsc::UnboundedReceiver<Vec<f32>>,
        result_tx: mpsc::UnboundedSender<TranscriptionMessage>,
    ) -> Result<(), SttError> {
        let url = format!(
            "{}?sample_rate={}&encoding=pcm&interim_results=true&language=ja&endpointing=5000",
            XAI_STT_WS_URL, sample_rate
        );

        let mut request = url
            .into_client_request()
            .map_err(|e| SttError::NetworkError(e.to_string()))?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", self.api_key).parse().map_err(
                |e: reqwest::header::InvalidHeaderValue| SttError::NetworkError(e.to_string()),
            )?,
        );

        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| SttError::NetworkError(format!("WebSocket connect failed: {}", e)))?;

        let (mut ws_sink, mut ws_read) = ws_stream.split();

        // transcript.created を待つ
        loop {
            match ws_read.next().await {
                Some(Ok(Message::Text(text))) => {
                    let event: WsEvent = serde_json::from_str(&text)
                        .map_err(|e| SttError::ParseError(e.to_string()))?;
                    if event.event_type == "transcript.created" {
                        break;
                    }
                    if event.event_type == "error" {
                        let msg = event.message.unwrap_or("Connection error".to_string());
                        return Err(SttError::ApiError(msg));
                    }
                }
                Some(Err(e)) => {
                    return Err(SttError::NetworkError(format!("WS recv error: {}", e)));
                }
                None => {
                    return Err(SttError::NetworkError(
                        "Connection closed before ready".to_string(),
                    ));
                }
                _ => {}
            }
        }

        let mut audio_done = false;
        let mut last_seen_text = String::new();

        loop {
            tokio::select! {
                chunk = audio_rx.recv(), if !audio_done => {
                    match chunk {
                        Some(samples) => {
                            let bytes = to_pcm16_bytes(&samples);
                            ws_sink.send(Message::Binary(bytes))
                                .await
                                .map_err(|e| SttError::NetworkError(e.to_string()))?;
                        }
                        None => {
                            ws_sink.send(Message::Text(
                                r#"{"type":"audio.done"}"#.to_string()
                            ))
                            .await
                            .map_err(|e| SttError::NetworkError(e.to_string()))?;
                            audio_done = true;
                        }
                    }
                }
                event_msg = ws_read.next() => {
                    match event_msg {
                        Some(Ok(Message::Text(text))) => {
                            let event: WsEvent = match serde_json::from_str(&text) {
                                Ok(e) => e,
                                Err(_) => continue,
                            };
                            match event.event_type.as_str() {
                                "transcript.partial" => {
                                    if let Some(ref t) = event.text {
                                        if !t.is_empty() {
                                            last_seen_text = t.clone();
                                        }
                                        let _ = result_tx.send(TranscriptionMessage::Partial(t.clone()));
                                    }
                                }
                                "transcript.done" => {
                                    let text = event.text
                                        .filter(|t| !t.is_empty())
                                        .unwrap_or_else(|| last_seen_text.clone());
                                    let _ = result_tx.send(TranscriptionMessage::Success(
                                        remove_punctuation(&text)
                                    ));
                                    return Ok(());
                                }
                                "error" => {
                                    let msg = event.message.unwrap_or("Unknown WS error".to_string());
                                    let _ = result_tx.send(TranscriptionMessage::Error(msg));
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                        None | Some(Err(_)) => {
                            if !last_seen_text.is_empty() {
                                let _ = result_tx.send(TranscriptionMessage::Success(
                                    remove_punctuation(&last_seen_text)
                                ));
                            } else {
                                let _ = result_tx.send(TranscriptionMessage::Error(
                                    "Connection closed unexpectedly".to_string()
                                ));
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn to_pcm16_bytes(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|&s| {
            let i = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            i.to_le_bytes()
        })
        .collect()
}

fn remove_punctuation(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '、' | '。' | '，' | '．' | '！' | '？' | ',' | '.' | '!' | '?' => ' ',
            _ => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_punctuation() {
        assert_eq!(remove_punctuation("こんにちは、世界。"), "こんにちは 世界");
        assert_eq!(remove_punctuation("Hello, world!"), "Hello world");
        assert_eq!(remove_punctuation("Yes? No."), "Yes No");
    }

    #[test]
    fn test_pcm_conversion() {
        let samples = vec![0.0f32, 1.0, -1.0, 0.5];
        let bytes = to_pcm16_bytes(&samples);
        assert_eq!(bytes.len(), 8);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 0i16);
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), i16::MAX);
        assert_eq!(i16::from_le_bytes([bytes[4], bytes[5]]), -i16::MAX);
    }
}
