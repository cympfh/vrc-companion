use rosc::encoder;
use rosc::{OscMessage, OscPacket, OscType};
use std::net::UdpSocket;

#[derive(Debug)]
pub enum VRChatError {
    SocketError(String),
    SendError(String),
}

impl std::fmt::Display for VRChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VRChatError::SocketError(msg) => write!(f, "Socket error: {}", msg),
            VRChatError::SendError(msg) => write!(f, "Send error: {}", msg),
        }
    }
}

impl std::error::Error for VRChatError {}

pub struct VRChatClient {
    pub target_addr: String,
}

impl VRChatClient {
    pub fn new() -> Self {
        Self {
            target_addr: "127.0.0.1:9000".to_string(),
        }
    }

    /// Send a message to VRChat via OSC (/chatbox/input)
    pub fn send_message(&self, message: &str) -> Result<(), VRChatError> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| VRChatError::SocketError(format!("Failed to bind socket: {}", e)))?;

        let packet = build_chatbox_packet(message);
        let msg_buf = encoder::encode(&packet)
            .map_err(|e| VRChatError::SendError(format!("Failed to encode OSC message: {}", e)))?;

        socket
            .send_to(&msg_buf, &self.target_addr)
            .map_err(|e| VRChatError::SendError(format!("Failed to send OSC message: {}", e)))?;

        Ok(())
    }
}

impl Default for VRChatClient {
    fn default() -> Self {
        Self::new()
    }
}

fn build_chatbox_packet(text: &str) -> OscPacket {
    OscPacket::Message(OscMessage {
        addr: "/chatbox/input".to_string(),
        args: vec![
            OscType::String(text.to_string()),
            OscType::Bool(true), // immediate
            OscType::Bool(true), // notify sound
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosc::decoder;

    #[test]
    fn test_build_chatbox_packet_roundtrip() {
        let packet = build_chatbox_packet("こんにちは");
        let encoded = encoder::encode(&packet).unwrap();
        let (_, decoded) = decoder::decode_udp(&encoded).unwrap();

        match decoded {
            OscPacket::Message(msg) => {
                assert_eq!(msg.addr, "/chatbox/input");
                assert_eq!(msg.args[0], OscType::String("こんにちは".to_string()));
                assert_eq!(msg.args[1], OscType::Bool(true));
                assert_eq!(msg.args[2], OscType::Bool(true));
            }
            _ => panic!("expected OscPacket::Message"),
        }
    }
}
