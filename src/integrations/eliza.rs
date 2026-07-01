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

#[derive(Debug, Serialize)]
struct TranslateRequest<'a> {
    source_lang: &'a str,
    target_lang: &'a str,
    text: &'a str,
}

#[derive(Debug, Deserialize)]
struct TranslateResponse {
    translated_text: String,
}

pub struct ElizaClient {
    url: String,
}

impl ElizaClient {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    fn chat_endpoint(&self) -> String {
        format!("{}/eliza/api/chat", self.url.trim_end_matches('/'))
    }

    fn translate_endpoint(&self) -> String {
        format!("{}/eliza/api/translate", self.url.trim_end_matches('/'))
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

    /// Send text to eliza-agent-server's translate endpoint and return the translated text
    pub fn translate(
        &self,
        source_lang: &str,
        target_lang: &str,
        text: &str,
    ) -> Result<String, String> {
        let request = TranslateRequest {
            source_lang,
            target_lang,
            text,
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        let response = client
            .post(self.translate_endpoint())
            .json(&request)
            .send()
            .map_err(|e| format!("Failed to send translate request to eliza: {}", e))?;

        let raw = response
            .text()
            .map_err(|e| format!("Failed to read eliza translate response: {}", e))?;

        let body: TranslateResponse = serde_json::from_str(&raw).map_err(|e| {
            format!(
                "Failed to parse eliza translate response: {}. Body was: {}",
                e, raw
            )
        })?;

        Ok(body.translated_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_endpoint_strips_trailing_slash() {
        let client = ElizaClient::new("http://localhost:9096/".to_string());
        assert_eq!(
            client.chat_endpoint(),
            "http://localhost:9096/eliza/api/chat"
        );
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

    #[test]
    fn test_translate_endpoint_strips_trailing_slash() {
        let client = ElizaClient::new("http://localhost:9096/".to_string());
        assert_eq!(
            client.translate_endpoint(),
            "http://localhost:9096/eliza/api/translate"
        );
    }

    #[test]
    fn test_translate_request_serialization() {
        let request = TranslateRequest {
            source_lang: "日本語",
            target_lang: "英語",
            text: "こんにちわ",
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            json,
            r#"{"source_lang":"日本語","target_lang":"英語","text":"こんにちわ"}"#
        );
    }

    #[test]
    fn test_translate_response_deserialization() {
        let raw = r#"{"translated_text":"Hello"}"#;
        let body: TranslateResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(body.translated_text, "Hello");
    }
}
