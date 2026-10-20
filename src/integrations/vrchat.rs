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
        self.send(&build_chatbox_packet(message))
    }

    /// アバターパラメーターを送る(`addr` は `/avatar/parameters/<名前>`)
    pub fn send_parameter(&self, addr: &str, value: OscType) -> Result<(), VRChatError> {
        self.send(&build_parameter_packet(addr, value))
    }

    fn send(&self, packet: &OscPacket) -> Result<(), VRChatError> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| VRChatError::SocketError(format!("Failed to bind socket: {}", e)))?;

        let msg_buf = encoder::encode(packet)
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

fn build_parameter_packet(addr: &str, value: OscType) -> OscPacket {
    OscPacket::Message(OscMessage {
        addr: addr.to_string(),
        args: vec![value],
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

    /// バイト列を直接確かめる: アドレスと型タグは 4 バイト境界まで 0 で埋め、値はビッグエンディアン
    #[test]
    fn test_parameter_packet_bytes() {
        let enc =
            |v| encoder::encode(&build_parameter_packet("/avatar/parameters/Hat", v)).unwrap();
        // "/avatar/parameters/Hat" は 22 バイト → 0 を足して 24
        let mut addr = b"/avatar/parameters/Hat".to_vec();
        addr.extend([0, 0]);
        let mut int = addr.clone();
        int.extend(b",i\0\0");
        int.extend(200i32.to_be_bytes());
        assert_eq!(enc(OscType::Int(200)), int);
        let mut float = addr.clone();
        float.extend(b",f\0\0");
        float.extend((-0.5f32).to_be_bytes());
        assert_eq!(enc(OscType::Float(-0.5)), float);
        let mut t = addr.clone();
        t.extend(b",T\0\0");
        assert_eq!(enc(OscType::Bool(true)), t);
        let mut f = addr.clone();
        f.extend(b",F\0\0");
        assert_eq!(enc(OscType::Bool(false)), f);
        assert!(enc(OscType::Int(1)).len() % 4 == 0);
    }
}
