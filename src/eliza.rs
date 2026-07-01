use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    messages: Vec<Message>,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatResponseMessage,
}

pub struct ElizaClient {
    url: String,
}

impl ElizaClient {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    fn chat_endpoint(&self) -> String {
        format!("{}/chat", self.url.trim_end_matches('/'))
    }

    /// Send transcribed text to eliza-agent-server and return the response message
    pub fn send_chat(&self, text: &str) -> Result<String, String> {
        let request = ChatRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: text.to_string(),
            }],
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        let response = client
            .post(self.chat_endpoint())
            .json(&request)
            .send()
            .map_err(|e| format!("Failed to send to eliza: {}", e))?;

        let raw = response
            .text()
            .map_err(|e| format!("Failed to read eliza response: {}", e))?;

        let body: ChatResponse = serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse eliza response: {}. Body was: {}", e, raw))?;

        Ok(body.message.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_endpoint_strips_trailing_slash() {
        let client = ElizaClient::new("http://localhost:9096/".to_string());
        assert_eq!(client.chat_endpoint(), "http://localhost:9096/chat");
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = ChatRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "こんにちは".to_string(),
            }],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            json,
            r#"{"messages":[{"role":"user","content":"こんにちは"}]}"#
        );
    }

    #[test]
    fn test_chat_response_deserialization() {
        let raw = r#"{"message":{"content":"元気だよ"}}"#;
        let body: ChatResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(body.message.content, "元気だよ");
    }
}
