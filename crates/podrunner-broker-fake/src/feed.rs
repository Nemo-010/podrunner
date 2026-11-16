//! The live console feed of the runner (T-310). In the flow of the results
//! service, the runner sends the lines of a running step only over a
//! websocket at `FeedStreamUrl` of the endpoint; with no such URL, a line
//! comes only when the log of its step is uploaded at the end of the step.
//! The broker takes the websocket on its loopback, and each text message
//! (`{"Value": [LINE...], "StepId": ID, "StartLine": N}`) gives its lines to
//! the live output at once.

use std::io::{Read, Write};

use base64::Engine;
use serde_json::Value;

/// The path of the feed on the broker.
pub const PATH: &str = "/_podrunner/feed";

/// The largest message that the broker takes: the runner sends 1 KiB chunks
/// of a batch of at most 500 lines.
const MESSAGE_MAX: usize = 4 << 20;

/// The value of `Sec-WebSocket-Accept` for the key of a client (RFC 6455).
pub fn accept(key: &str) -> String {
    let mut text = key.trim().as_bytes().to_vec();
    text.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let d = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &text);
    base64::engine::general_purpose::STANDARD.encode(d.as_ref())
}

/// The answer to the upgrade of a client.
pub fn handshake(w: &mut impl Write, key: &str) -> std::io::Result<()> {
    write!(
        w,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept(key)
    )?;
    w.flush()
}

/// Read each message of the client until it closes; `each` gets the step
/// and the lines of each text message.
pub fn read_messages(r: &mut impl Read, mut each: impl FnMut(&str, Vec<String>)) {
    let mut message = Vec::new();
    loop {
        let mut head = [0u8; 2];
        if r.read_exact(&mut head).is_err() {
            return;
        }
        let (fin, opcode, masked) = (head[0] & 0x80 != 0, head[0] & 0x0f, head[1] & 0x80 != 0);
        let len = match head[1] & 0x7f {
            126 => {
                let mut b = [0u8; 2];
                if r.read_exact(&mut b).is_err() {
                    return;
                }
                u64::from(u16::from_be_bytes(b))
            }
            127 => {
                let mut b = [0u8; 8];
                if r.read_exact(&mut b).is_err() {
                    return;
                }
                u64::from_be_bytes(b)
            }
            n => u64::from(n),
        };
        let mut mask = [0u8; 4];
        if masked && r.read_exact(&mut mask).is_err() {
            return;
        }
        if len as usize > MESSAGE_MAX || message.len() + len as usize > MESSAGE_MAX {
            return;
        }
        let mut data = vec![0u8; len as usize];
        if r.read_exact(&mut data).is_err() {
            return;
        }
        if masked {
            for (i, b) in data.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        match opcode {
            // Close.
            0x8 => return,
            // Text or a continuation; ping and pong carry no lines.
            0x0..=0x2 => message.extend_from_slice(&data),
            _ => continue,
        }
        if fin {
            if let Some((step, lines)) = parse(&message) {
                each(&step, lines);
            }
            message.clear();
        }
    }
}

/// The step and the lines of one message.
fn parse(message: &[u8]) -> Option<(String, Vec<String>)> {
    let v: Value = serde_json::from_slice(message).ok()?;
    let get = |k: &str| v.get(k).or_else(|| v.get(k.to_ascii_lowercase()));
    let step = get("StepId").and_then(Value::as_str).or_else(|| v.get("stepId").and_then(Value::as_str))?;
    let lines = get("Value").or_else(|| v.get("value"))?.as_array()?;
    Some((step.to_ascii_lowercase(), lines.iter().filter_map(Value::as_str).map(String::from).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example key of RFC 6455 gives its accept value.
    #[test]
    fn the_accept_of_the_rfc() {
        assert_eq!(accept("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    /// Two masked chunks of one message give its lines; a close ends.
    #[test]
    fn a_message_in_two_masked_frames() {
        let text = br#"{"Value":["one","two"],"StepId":"AB-1","StartLine":1}"#;
        let (a, b) = text.split_at(10);
        let frame = |fin: bool, op: u8, data: &[u8]| {
            let mask = [1u8, 2, 3, 4];
            let mut f = vec![if fin { 0x80 } else { 0 } | op, 0x80 | data.len() as u8];
            f.extend_from_slice(&mask);
            f.extend(data.iter().enumerate().map(|(i, x)| x ^ mask[i % 4]));
            f
        };
        let mut wire = frame(false, 1, a);
        wire.extend(frame(true, 0, b));
        wire.extend(frame(true, 8, b""));
        let mut got = Vec::new();
        read_messages(&mut &wire[..], |s, l| got.push((s.to_string(), l)));
        assert_eq!(got, [("ab-1".to_string(), vec!["one".to_string(), "two".to_string()])]);
    }
}
