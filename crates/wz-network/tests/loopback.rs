//! 로컬 루프백으로 프레이밍/RC4/로그인 상태 머신을 검증한다.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::thread;
use wz_network::pack::Packet;
use wz_network::session::{read_frame, write_frame, ClientSession, ClientState};
use wz_network::Rc4;

fn b64(data: &[u8]) -> String {
    // 테스트용 간단 인코더 (RFC4648)
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[test]
fn handshake_and_login_over_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut read = None;
        let mut write = None;
        let req = read_frame(&mut stream, &mut read).unwrap().unwrap();
        assert!(String::from_utf8_lossy(&req).contains("\"$type\":\"1\""));
        let resp = format!(
            "{{\"$type\":\"2\",\"KeyEncryptedS2C\":\"{}\",\"KeyEncryptedC2S\":\"{}\"}}",
            b64(b"s2c-key"),
            b64(b"c2s-key")
        );
        write_frame(&mut stream, &mut write, resp.as_bytes()).unwrap();
        read = Rc4::new(b"c2s-key");
        write = Rc4::new(b"s2c-key");
        let login = read_frame(&mut stream, &mut read).unwrap().unwrap();
        let text = String::from_utf8_lossy(&login).to_string();
        assert!(text.contains("\"$type\":\"3\""));
        assert!(text.contains("\"WcID\":\"wc-01\""));
        let login_resp = br#"{"$type":"4","SessionID":"sess-42"}"#;
        write_frame(&mut stream, &mut write, login_resp).unwrap();
        stream.flush().unwrap();
        text
    });

    let stream = TcpStream::connect(addr).unwrap();
    let mut client = ClientSession::new(stream);
    assert_eq!(client.state, ClientState::Connected);

    client.send_crypt_req(&[1, 0, 1], &[9, 9]).unwrap();
    assert_eq!(client.state, ClientState::CryptoRequested);

    let body = client.recv_packet().unwrap().unwrap();
    let identity = |data: &[u8]| Some(data.to_vec());
    assert!(client.handle_crypt_resp(&body, &identity).unwrap());
    assert_eq!(client.state, ClientState::Ready);
    assert_eq!(client.server_key.as_deref(), Some(&b"s2c-key"[..]));
    assert_eq!(client.client_key.as_deref(), Some(&b"c2s-key"[..]));

    client.send_login("wc-01", "테스터").unwrap();
    assert_eq!(client.state, ClientState::LoginSent);
    let body = client.recv_packet().unwrap().unwrap();
    assert!(client.handle_login_resp(&body));
    assert_eq!(client.state, ClientState::LoggedIn);

    let seen = server.join().unwrap();
    assert!(seen.contains("\"NickName\":\"테스터\""));
}

#[test]
fn packet_codec_type_ids_are_stable() {
    assert!(Packet::Heartbeat.json().contains("\"$type\":\"0\""));
    assert!(Packet::LoginReq {
        wc_id: "a",
        nick_name: "b"
    }
    .json()
    .contains("\"$type\":\"3\""));
}
