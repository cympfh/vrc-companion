use rosc::decoder;
use rosc::encoder;
use rosc::{OscMessage, OscPacket, OscType};
use std::net::UdpSocket;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

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

/// MuteSelf の False→True 切り替えを検出する。
/// True→False (ミュート解除) の直後 `timeout` 以内に False→True (ミュート) が来たら「くいくい」とみなす。
pub struct MuteToggleDetector {
    unmute_time: Option<Instant>,
    timeout: Duration,
}

impl MuteToggleDetector {
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(1))
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            unmute_time: None,
            timeout,
        }
    }

    /// 現在の MuteSelf 値を渡す。トリガー条件を満たしたら true を返す。
    pub fn on_mute_changed(&mut self, is_muted: bool) -> bool {
        if !is_muted {
            self.unmute_time = Some(Instant::now());
            false
        } else if let Some(unmute_time) = self.unmute_time.take() {
            unmute_time.elapsed() <= self.timeout
        } else {
            false
        }
    }
}

impl Default for MuteToggleDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// VRChat から OSC (port=9001) で MuteSelf パラメータを受信し、
/// 1秒以内に False→True と切り替わったら sender にトリガーを送信する
pub fn start_mute_listener(sender: Sender<()>) {
    std::thread::spawn(move || {
        let socket = match UdpSocket::bind("0.0.0.0:9001") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[VRChat OSC Listener] Failed to bind port 9001: {}", e);
                return;
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok();
        println!("[VRChat OSC Listener] Listening on port 9001 for MuteSelf parameter");

        let mut buf = [0u8; 65535];
        let mut detector = MuteToggleDetector::new();

        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, _addr)) => {
                    if let Ok((_, OscPacket::Message(msg))) = decoder::decode_udp(&buf[..size])
                        && msg.addr == "/avatar/parameters/MuteSelf"
                    {
                        let is_muted = match msg.args.first() {
                            Some(OscType::Bool(b)) => *b,
                            Some(OscType::Int(i)) => *i != 0,
                            Some(OscType::Float(f)) => *f != 0.0,
                            _ => continue,
                        };
                        println!("[VRChat OSC Listener] MuteSelf={}", is_muted);

                        if detector.on_mute_changed(is_muted) {
                            println!(
                                "[VRChat OSC Listener] Mute toggle detected → trigger recording"
                            );
                            if sender.send(()).is_err() {
                                break;
                            }
                        }
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    eprintln!("[VRChat OSC Listener] recv error: {}", e);
                    break;
                }
            }
        }
        println!("[VRChat OSC Listener] Stopped");
    });
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

    #[test]
    fn test_mute_toggle_no_trigger_without_prior_unmute() {
        let mut detector = MuteToggleDetector::new();
        assert!(!detector.on_mute_changed(true));
    }

    #[test]
    fn test_mute_toggle_no_trigger_on_unmute_alone() {
        let mut detector = MuteToggleDetector::new();
        assert!(!detector.on_mute_changed(false));
    }

    #[test]
    fn test_mute_toggle_triggers_immediately_after_unmute() {
        let mut detector = MuteToggleDetector::new();
        assert!(!detector.on_mute_changed(false));
        assert!(detector.on_mute_changed(true));
    }

    #[test]
    fn test_mute_toggle_no_trigger_after_timeout() {
        let mut detector = MuteToggleDetector::with_timeout(Duration::from_millis(5));
        assert!(!detector.on_mute_changed(false));
        std::thread::sleep(Duration::from_millis(15));
        assert!(!detector.on_mute_changed(true));
    }

    #[test]
    fn test_mute_toggle_consumes_unmute_time() {
        let mut detector = MuteToggleDetector::new();
        assert!(!detector.on_mute_changed(false));
        assert!(detector.on_mute_changed(true));
        // 直前の unmute は消費済みなので、続けて true が来ても再トリガーしない
        assert!(!detector.on_mute_changed(true));
    }

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
