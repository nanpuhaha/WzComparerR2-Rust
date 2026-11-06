//! `WcClient`의 TCP 프레이밍/암호화/로그인 상태 머신.
//!
//! - 프레임: `uint16 LE 길이 + 본문` (암호화가 켜지면 길이까지 RC4 대상)
//! - 핸드셰이크: CryptReq → CryptResp(키 복호화) → RC4 시작 → Login
//!
//! RSA 복호화는 [`KeyDecryptor`]로 주입한다. 원본은 `RSACryptoServiceProvider`
//! 를 쓰지만 macOS 오프라인 테스트에서는 결정적 복호화기를 주입해 흐름을
//! 검증한다.

use crate::pack::Packet;
use crate::Rc4;
use serde_json::Value;
use std::io::{self, Read, Write};

/// `PackCryptResp`의 암호화된 키를 푸는 함수.
pub type KeyDecryptor<'a> = dyn Fn(&[u8]) -> Option<Vec<u8>> + 'a;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    Connected,
    CryptoRequested,
    Ready,
    LoginSent,
    LoggedIn,
}

/// 한쪽 방향 프레임 읽기. RC4가 있으면 길이부터 복호화한다.
pub fn read_frame<R: Read>(io: &mut R, rc4: &mut Option<Rc4>) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 2];
    match io.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    if let Some(cipher) = rc4 {
        cipher.apply(&mut len_buf);
    }
    let len = u16::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    io.read_exact(&mut body)?;
    if let Some(cipher) = rc4 {
        cipher.apply(&mut body);
    }
    Ok(Some(body))
}

/// 한쪽 방향 프레임 쓰기.
pub fn write_frame<W: Write>(io: &mut W, rc4: &mut Option<Rc4>, body: &[u8]) -> io::Result<()> {
    let len = u16::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "packet too large"))?;
    let mut frame = Vec::with_capacity(body.len() + 2);
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(body);
    if let Some(cipher) = rc4 {
        cipher.apply(&mut frame);
    }
    io.write_all(&frame)?;
    io.flush()
}

/// 클라이언트 세션 상태 머신.
pub struct ClientSession<S: Read + Write> {
    io: S,
    read_rc4: Option<Rc4>,
    write_rc4: Option<Rc4>,
    pub state: ClientState,
    /// 핸드셰이크에서 받은 서버→클라이언트 키 (복호화 후).
    pub server_key: Option<Vec<u8>>,
    pub client_key: Option<Vec<u8>>,
}

impl<S: Read + Write> ClientSession<S> {
    pub fn new(io: S) -> Self {
        Self {
            io,
            read_rc4: None,
            write_rc4: None,
            state: ClientState::Connected,
            server_key: None,
            client_key: None,
        }
    }

    pub fn send_packet(&mut self, packet: &Packet) -> io::Result<()> {
        write_frame(&mut self.io, &mut self.write_rc4, packet.json().as_bytes())
    }

    pub fn recv_packet(&mut self) -> io::Result<Option<Vec<u8>>> {
        read_frame(&mut self.io, &mut self.read_rc4)
    }

    /// `CryptoRequest` — 공개키를 보낸다.
    pub fn send_crypt_req(&mut self, exponent: &[u8], modulus: &[u8]) -> io::Result<()> {
        self.send_packet(&Packet::CryptReq { exponent, modulus })?;
        self.state = ClientState::CryptoRequested;
        Ok(())
    }

    /// `OnPackReceived(PackCryptResp)` — 키를 풀고 RC4를 시작한다.
    pub fn handle_crypt_resp(
        &mut self,
        body: &[u8],
        decrypt: &KeyDecryptor<'_>,
    ) -> io::Result<bool> {
        let json: Value = serde_json::from_slice(body)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if json.get("$type").and_then(Value::as_str) != Some("2") {
            return Ok(false);
        }
        let s2c = decode_key(&json, "KeyEncryptedS2C", decrypt)?;
        let c2s = decode_key(&json, "KeyEncryptedC2S", decrypt)?;
        self.read_rc4 = Rc4::new(&s2c);
        self.write_rc4 = Rc4::new(&c2s);
        self.server_key = Some(s2c);
        self.client_key = Some(c2s);
        self.state = ClientState::Ready;
        Ok(true)
    }

    /// 서버 상태/로그인 요청. 원본 `ServerInfoRequest` + `LoginRequest` 순서.
    pub fn send_login(&mut self, wc_id: &str, nick_name: &str) -> io::Result<()> {
        self.send_packet(&Packet::LoginReq { wc_id, nick_name })?;
        self.state = ClientState::LoginSent;
        Ok(())
    }

    /// `PackLoginResp` 처리.
    pub fn handle_login_resp(&mut self, body: &[u8]) -> bool {
        let Ok(json) = serde_json::from_slice::<Value>(body) else {
            return false;
        };
        if json.get("$type").and_then(Value::as_str) != Some("4") {
            return false;
        }
        self.state = ClientState::LoggedIn;
        true
    }
}

fn decode_key(json: &Value, field: &str, decrypt: &KeyDecryptor<'_>) -> io::Result<Vec<u8>> {
    let text = json
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing key"))?;
    let encrypted = base64_decode(text)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad base64"))?;
    decrypt(&encrypted)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "key decrypt failed"))
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let bytes: Vec<u8> = text.bytes().filter(|b| !b"=\r\n".contains(b)).collect();
    if bytes.len() % 4 == 1 {
        return None;
    }
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            let v = B64.iter().position(|c| c == b)? as u32;
            n |= v << (18 - i * 6);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn frame_round_trip_plain_and_rc4() {
        for use_rc4 in [false, true] {
            let mut buf = Vec::new();
            let mut w = if use_rc4 { Rc4::new(b"key1") } else { None };
            write_frame(&mut buf, &mut w, b"hello").unwrap();
            let mut r = if use_rc4 { Rc4::new(b"key1") } else { None };
            let mut cursor = Cursor::new(buf);
            assert_eq!(
                read_frame(&mut cursor, &mut r).unwrap().as_deref(),
                Some(&b"hello"[..])
            );
            assert!(read_frame(&mut cursor, &mut r).unwrap().is_none());
        }
    }

    #[test]
    fn base64_decode_round_trip() {
        assert_eq!(base64_decode("AQID").unwrap(), vec![1, 2, 3]);
        assert_eq!(base64_decode("AQI=").unwrap(), vec![1, 2]);
        assert_eq!(base64_decode("").unwrap(), Vec::<u8>::new());
    }
}

/// 재접속 백오프. 원본은 실패 시 5초 고정, 성공 후 재접속 시도는 즉시였다.
/// 여기서는 시도별 지연을 명시적으로 계산해 테스트 가능하게 한다.
#[derive(Debug, Clone)]
pub struct Backoff {
    pub initial: std::time::Duration,
    pub factor: f64,
    pub max: std::time::Duration,
    pub max_attempts: usize,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            initial: std::time::Duration::from_secs(5),
            factor: 2.0,
            max: std::time::Duration::from_secs(60),
            max_attempts: 10,
        }
    }
}

impl Backoff {
    /// `attempt`는 0부터. 한도를 넘으면 `None`.
    pub fn delay(&self, attempt: usize) -> Option<std::time::Duration> {
        if self.max_attempts == 0 || attempt >= self.max_attempts {
            return None;
        }
        let millis = self.initial.as_millis() as f64 * self.factor.powi(attempt as i32);
        let capped = millis.min(self.max.as_millis() as f64);
        Some(std::time::Duration::from_millis(capped as u64))
    }
}

/// 재접속을 포함한 클라이언트. `connect`는 새 스트림을 만든다.
pub struct ReconnectingClient<C, S>
where
    C: FnMut() -> io::Result<S>,
    S: Read + Write,
{
    connector: C,
    session: Option<ClientSession<S>>,
    pub backoff: Backoff,
    pub attempts: usize,
    pub reconnects: usize,
}

impl<C, S> ReconnectingClient<C, S>
where
    C: FnMut() -> io::Result<S>,
    S: Read + Write,
{
    pub fn new(connector: C, backoff: Backoff) -> Self {
        Self {
            connector,
            session: None,
            backoff,
            attempts: 0,
            reconnects: 0,
        }
    }

    pub fn session(&self) -> Option<&ClientSession<S>> {
        self.session.as_ref()
    }

    /// 백오프를 적용해 연결한다.
    pub fn connect(&mut self) -> io::Result<()> {
        let mut attempt = 0usize;
        loop {
            match (self.connector)() {
                Ok(io) => {
                    self.session = Some(ClientSession::new(io));
                    self.attempts = attempt;
                    return Ok(());
                }
                Err(e) => {
                    self.attempts = attempt + 1;
                    match self.backoff.delay(attempt) {
                        Some(delay) => {
                            std::thread::sleep(delay);
                            attempt += 1;
                        }
                        None => return Err(e),
                    }
                }
            }
        }
    }

    /// 하트비트는 즉시 되돌려 보내고, 그 외 패킷만 반환한다.
    pub fn next_packet(&mut self) -> io::Result<Option<Vec<u8>>> {
        loop {
            let Some(session) = self.session.as_mut() else {
                self.reconnect()?;
                continue;
            };
            match session.recv_packet() {
                Ok(Some(body)) => {
                    if body.starts_with(b"{\"$type\":\"0\"}") {
                        session.send_packet(&Packet::Heartbeat)?;
                        continue;
                    }
                    return Ok(Some(body));
                }
                Ok(None) => {
                    self.reconnect()?;
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    self.reconnect()?;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn reconnect(&mut self) -> io::Result<()> {
        if self.backoff.max_attempts == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "reconnect disabled",
            ));
        }
        self.reconnects += 1;
        self.session = None;
        self.connect()
    }
}
