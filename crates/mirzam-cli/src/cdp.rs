//! A Chrome DevTools Protocol client, exactly as much of one as `export
//! video` needs to film a deck: launch a headless Chromium, attach to one
//! tab, and send commands — `Page.captureScreenshot`, thirty times a
//! second, being the point of the whole exercise.
//!
//! Hand-written rather than pulled in, for the same reason the PowerPoint
//! packaging is: the crates that do this drag an async runtime and a TLS
//! stack into a CLI that needs neither. What the protocol actually requires
//! here is a WebSocket *client* on a loopback socket — no TLS, no server
//! role, text frames of JSON — which is a couple hundred lines of RFC 6455
//! against `std::net`, plus the base64 the handshake and the screencast
//! frames are wrapped in.
//!
//! The port is never guessed: Chromium is launched with
//! `--remote-debugging-port=0` and a private `--user-data-dir`, and writes
//! the port it actually bound — and the browser target's path — into
//! `DevToolsActivePort` inside that directory. Polling for that file is the
//! documented handshake, and the private directory means a running personal
//! Chrome is never touched.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a single command may wait for its reply. Generous: everything
/// this client sends is answered in milliseconds by a local process, so a
/// silence this long means the browser is gone, not busy.
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// A headless Chromium this client launched, and the connection to it. The
/// browser and its temporary profile die with this value.
pub(crate) struct Browser {
    child: std::process::Child,
    user_data: PathBuf,
    pub(crate) cdp: Cdp,
}

impl Browser {
    /// Launches `bin` headless with a fresh profile and connects. `width` and
    /// `height` size the window; the recorder overrides the viewport through
    /// `Emulation.setDeviceMetricsOverride` anyway, so they only need to be
    /// roomy enough.
    pub(crate) fn launch(bin: &str, width: u32, height: u32) -> Result<Browser, String> {
        let user_data = std::env::temp_dir().join(format!(
            "mirzam-record-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&user_data)
            .map_err(|e| format!("cannot make a browser profile directory: {e}"))?;
        let mut child = std::process::Command::new(bin)
            .args([
                "--headless",
                "--disable-gpu",
                "--no-sandbox",
                "--hide-scrollbars",
                // A filmed deck has nobody to click: a clip marked autoplay
                // must actually start.
                "--autoplay-policy=no-user-gesture-required",
                "--remote-debugging-port=0",
                &format!("--user-data-dir={}", user_data.display()),
                &format!("--window-size={width},{height}"),
                "about:blank",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot run {bin}: {e}"))?;

        let port_file = user_data.join("DevToolsActivePort");
        let deadline = Instant::now() + Duration::from_secs(20);
        let (port, browser_path) = loop {
            if let Some(parsed) = std::fs::read_to_string(&port_file)
                .ok()
                .and_then(|s| parse_active_port(&s))
            {
                break parsed;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = std::fs::remove_dir_all(&user_data);
                return Err(format!(
                    "{bin} never wrote DevToolsActivePort; is it a Chromium?"
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let cdp = match Cdp::connect(port, &browser_path) {
            Ok(cdp) => cdp,
            Err(e) => {
                let _ = child.kill();
                let _ = std::fs::remove_dir_all(&user_data);
                return Err(format!("cannot reach Chromium's DevTools socket: {e}"));
            }
        };
        Ok(Browser {
            child,
            user_data,
            cdp,
        })
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        // Kill rather than `Browser.close`: by the time this runs the socket
        // may already be gone, and the process must not outlive the export
        // either way.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.user_data);
    }
}

/// `DevToolsActivePort`: the bound port on the first line, the browser
/// target's WebSocket path on the second.
fn parse_active_port(s: &str) -> Option<(u16, String)> {
    let mut lines = s.lines();
    let port = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.trim();
    if path.is_empty() {
        return None;
    }
    Some((port, path.to_string()))
}

/// The connection: commands go out under a lock, replies and events come
/// back on a reader thread — replies routed to their waiting caller by id,
/// events queued for whoever is filming.
pub(crate) struct Cdp {
    writer: Arc<Mutex<TcpStream>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>,
    /// Protocol events, in arrival order. `recv_timeout` on this is the
    /// recorder's main loop.
    pub(crate) events: mpsc::Receiver<Value>,
    next_id: AtomicU64,
    /// The attached tab's session, once `attach` has run: every later
    /// command is addressed to it.
    session: Mutex<Option<String>>,
}

impl Cdp {
    fn connect(port: u16, path: &str) -> Result<Cdp, String> {
        let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
        ws_handshake(&stream, port, path)?;
        let reader = stream.try_clone().map_err(|e| e.to_string())?;
        let writer = Arc::new(Mutex::new(stream));
        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>> = Default::default();
        let (event_tx, events) = mpsc::channel();
        let w2 = Arc::clone(&writer);
        let p2 = Arc::clone(&pending);
        std::thread::spawn(move || read_loop(reader, w2, p2, event_tx));
        Ok(Cdp {
            writer,
            pending,
            events,
            next_id: AtomicU64::new(1),
            session: Mutex::new(None),
        })
    }

    /// Creates a tab and attaches to it; every later `call` is addressed
    /// there. Returns nothing a caller needs — the session lives in here.
    pub(crate) fn attach(&self) -> Result<(), String> {
        let target = self.call("Target.createTarget", json!({"url": "about:blank"}))?;
        let target_id = target["targetId"]
            .as_str()
            .ok_or("Target.createTarget returned no targetId")?
            .to_string();
        let attached = self.call(
            "Target.attachToTarget",
            json!({"targetId": target_id, "flatten": true}),
        )?;
        let session = attached["sessionId"]
            .as_str()
            .ok_or("Target.attachToTarget returned no sessionId")?
            .to_string();
        *self.session.lock().unwrap() = Some(session);
        Ok(())
    }

    /// Sends a command and waits for its reply. The reply's `result` comes
    /// back; a protocol error becomes an `Err` naming the method.
    pub(crate) fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.send(id, method, params)?;
        let reply = rx.recv_timeout(REPLY_TIMEOUT).map_err(|_| {
            self.pending.lock().unwrap().remove(&id);
            format!("{method}: the browser stopped answering")
        })?;
        if let Some(err) = reply.get("error") {
            return Err(format!(
                "{method}: {}",
                err["message"].as_str().unwrap_or("protocol error")
            ));
        }
        Ok(reply["result"].clone())
    }

    /// Sends a command whose reply nobody needs — the screencast ack, sixty
    /// times a second. The reply still arrives and is dropped by the router.
    pub(crate) fn call_no_wait(&self, method: &str, params: Value) -> Result<(), String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.send(id, method, params)
    }

    /// Evaluates one JavaScript expression in the tab, by value.
    pub(crate) fn eval(&self, expression: &str) -> Result<Value, String> {
        let r = self.call(
            "Runtime.evaluate",
            json!({"expression": expression, "returnByValue": true}),
        )?;
        if let Some(text) = r["exceptionDetails"]["exception"]["description"].as_str() {
            return Err(format!("the page threw: {text}"));
        }
        Ok(r["result"]["value"].clone())
    }

    fn send(&self, id: u64, method: &str, params: Value) -> Result<(), String> {
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = self.session.lock().unwrap().as_ref() {
            msg["sessionId"] = Value::String(s.clone());
        }
        let bytes = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
        let mut w = self.writer.lock().unwrap();
        ws_write(&mut *w, 0x1, &bytes).map_err(|e| format!("{method}: socket write failed: {e}"))
    }
}

/// Reads WebSocket messages forever: replies are routed to their caller by
/// id, events are queued, pings are answered, and when the socket dies a
/// `__closed` pseudo-event tells the recorder the browser is gone.
fn read_loop(
    mut reader: TcpStream,
    writer: Arc<Mutex<TcpStream>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>,
    events: mpsc::Sender<Value>,
) {
    while let Ok((opcode, payload)) = ws_read(&mut reader) {
        match opcode {
            0x1 | 0x2 => {
                let Ok(msg) = serde_json::from_slice::<Value>(&payload) else {
                    continue;
                };
                match msg.get("id").and_then(Value::as_u64) {
                    Some(id) => {
                        if let Some(tx) = pending.lock().unwrap().remove(&id) {
                            let _ = tx.send(msg);
                        }
                        // No waiter: a no-wait command's reply. Dropped.
                    }
                    None => {
                        if events.send(msg).is_err() {
                            break;
                        }
                    }
                }
            }
            0x9 => {
                let mut w = writer.lock().unwrap();
                if ws_write(&mut *w, 0xA, &payload).is_err() {
                    break;
                }
            }
            0x8 => break,
            _ => {}
        }
    }
    let _ = events.send(json!({"method": "__closed"}));
}

/// The HTTP upgrade. The server's `Sec-WebSocket-Accept` is not verified:
/// its whole purpose is to catch confused proxies and cross-protocol
/// requests, and there is no proxy on a loopback socket this process just
/// opened to a port it just read from a file it owns.
fn ws_handshake(mut stream: &TcpStream, port: u16, path: &str) -> Result<(), String> {
    let key = base64(&nonce16());
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Upgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    // Read headers to their blank line, one byte at a time: whatever follows
    // is the first WebSocket frame and must not be swallowed by a buffer.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 16 * 1024 {
            return Err("the DevTools endpoint never finished its handshake".into());
        }
        stream.read_exact(&mut byte).map_err(|e| e.to_string())?;
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head);
    if !head.starts_with("HTTP/1.1 101") {
        return Err(format!(
            "the DevTools endpoint refused the upgrade: {}",
            head.lines().next().unwrap_or("")
        ));
    }
    Ok(())
}

/// Reads one complete message, following continuation frames to their FIN.
fn ws_read(r: &mut impl Read) -> std::io::Result<(u8, Vec<u8>)> {
    let mut message: Vec<u8> = Vec::new();
    let mut opcode = 0u8;
    loop {
        let mut hdr = [0u8; 2];
        r.read_exact(&mut hdr)?;
        let fin = hdr[0] & 0x80 != 0;
        let op = hdr[0] & 0x0F;
        let masked = hdr[1] & 0x80 != 0;
        let mut len = u64::from(hdr[1] & 0x7F);
        if len == 126 {
            let mut ext = [0u8; 2];
            r.read_exact(&mut ext)?;
            len = u64::from(u16::from_be_bytes(ext));
        } else if len == 127 {
            let mut ext = [0u8; 8];
            r.read_exact(&mut ext)?;
            len = u64::from_be_bytes(ext);
        }
        let mask = if masked {
            let mut m = [0u8; 4];
            r.read_exact(&mut m)?;
            Some(m)
        } else {
            None
        };
        let start = message.len();
        message.resize(start + len as usize, 0);
        r.read_exact(&mut message[start..])?;
        if let Some(m) = mask {
            for (i, b) in message[start..].iter_mut().enumerate() {
                *b ^= m[i % 4];
            }
        }
        if op != 0 {
            opcode = op;
        }
        if fin {
            // A control frame may arrive between fragments of a message; the
            // only one Chromium sends unprompted is a ping, and it never
            // fragments its JSON in practice — but handle the interleaving
            // by returning the control frame alone when nothing is pending.
            if (op & 0x8) != 0 && start > 0 {
                let ctrl = message.split_off(start);
                // Put the data fragments back to wait for their FIN? RFC
                // 6455 forbids interleaving *data* frames, so `message` can
                // only be an unfinished fragmented data message and `ctrl`
                // the control frame that jumped the queue. Answering the
                // control frame first is exactly what the RFC asks for.
                return Ok((op, ctrl));
            }
            return Ok((opcode, message));
        }
    }
}

/// Writes one frame, masked as a client must.
fn ws_write(w: &mut impl Write, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | opcode);
    let len = payload.len();
    if len < 126 {
        frame.push(0x80 | len as u8);
    } else if len <= u16::MAX as usize {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    let mask = nonce16()[..4]
        .try_into()
        .unwrap_or([0x37, 0xFA, 0x21, 0x3D]);
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    w.write_all(&frame)?;
    w.flush()
}

/// Sixteen bytes that only need to be different each time, not secret: the
/// WebSocket mask and key exist to defeat caching proxies, and this socket
/// never crosses one.
fn nonce16() -> [u8; 16] {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut seed = now.as_nanos() ^ (u128::from(std::process::id()) << 64);
    let mut out = [0u8; 16];
    for b in &mut out {
        // xorshift, enough churn to differ per call.
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        *b = seed as u8;
    }
    out
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, for the handshake key.
pub(crate) fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Standard base64 back to bytes, for the screencast frames. Whitespace is
/// skipped; anything else out of alphabet is an error.
pub(crate) fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u8;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' => continue,
            _ => return Err(format!("not base64: byte {c:#x}")),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for data in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            &[0u8, 255, 128, 7, 66][..],
        ] {
            let enc = base64(data);
            assert_eq!(base64_decode(&enc).unwrap(), data, "via {enc}");
        }
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_decode_skips_line_breaks_and_rejects_junk() {
        assert_eq!(base64_decode("Zm9v\r\nYmFy").unwrap(), b"foobar");
        assert!(base64_decode("Zm9v!").is_err());
    }

    #[test]
    fn websocket_frames_round_trip_through_read() {
        // A client-masked frame is what we write; servers send unmasked, so
        // exercise both paths by decoding our own write and a hand-built
        // server frame.
        let mut written = Vec::new();
        ws_write(&mut written, 0x1, b"hello").unwrap();
        let (op, payload) = ws_read(&mut &written[..]).unwrap();
        assert_eq!((op, payload.as_slice()), (0x1, &b"hello"[..]));

        let mut server = vec![0x81, 3];
        server.extend_from_slice(b"abc");
        let (op, payload) = ws_read(&mut &server[..]).unwrap();
        assert_eq!((op, payload.as_slice()), (0x1, &b"abc"[..]));
    }

    #[test]
    fn websocket_read_reassembles_fragments_and_extended_lengths() {
        // Two fragments: text without FIN, then a continuation with it.
        let mut wire = vec![0x01, 2, b'h', b'i'];
        wire.extend_from_slice(&[0x80, 3, b'y', b'o', b'u']);
        let (op, payload) = ws_read(&mut &wire[..]).unwrap();
        assert_eq!((op, payload.as_slice()), (0x1, &b"hiyou"[..]));

        // A 300-byte payload takes the 16-bit length form.
        let body = vec![b'x'; 300];
        let mut wire = vec![0x81, 126];
        wire.extend_from_slice(&300u16.to_be_bytes());
        wire.extend_from_slice(&body);
        let (op, payload) = ws_read(&mut &wire[..]).unwrap();
        assert_eq!((op, payload.len()), (0x1, 300));
    }

    #[test]
    fn active_port_file_parses() {
        assert_eq!(
            parse_active_port("39131\n/devtools/browser/ab-cd\n"),
            Some((39131, "/devtools/browser/ab-cd".into()))
        );
        assert_eq!(parse_active_port(""), None);
        assert_eq!(parse_active_port("39131\n"), None);
    }
}
