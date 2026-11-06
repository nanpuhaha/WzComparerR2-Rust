//! 재접속/백오프와 하트비트 에코 루프백 테스트.

use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;
use wz_network::pack::Packet;
use wz_network::session::{read_frame, write_frame, Backoff, ReconnectingClient};

#[test]
fn heartbeat_echo_and_reconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        // 1) 첫 연결: 하트비트 전송 후 에코 확인, 연결 종료
        {
            let (mut stream, _) = listener.accept().unwrap();
            let mut read = None;
            let mut write = None;
            write_frame(&mut stream, &mut write, Packet::Heartbeat.json().as_bytes()).unwrap();
            let echo = read_frame(&mut stream, &mut read).unwrap().unwrap();
            assert!(String::from_utf8_lossy(&echo).contains("\"$type\":\"0\""));
        }
        // 2) 재접속: 로그인 응답 전송
        let (mut stream, _) = listener.accept().unwrap();
        let mut write = None;
        write_frame(
            &mut stream,
            &mut write,
            br#"{"$type":"4","SessionID":"after-reconnect"}"#,
        )
        .unwrap();
    });

    let backoff = Backoff {
        initial: Duration::from_millis(1),
        factor: 1.0,
        max: Duration::from_millis(1),
        max_attempts: 3,
    };
    let mut client = ReconnectingClient::new(move || TcpStream::connect(addr), backoff);
    client.connect().unwrap();
    let packet = client.next_packet().unwrap().unwrap();
    let text = String::from_utf8_lossy(&packet);
    assert!(text.contains("\"$type\":\"4\""));
    assert!(text.contains("after-reconnect"));
    assert!(client.reconnects >= 1, "reconnects={}", client.reconnects);
    server.join().unwrap();
}

#[test]
fn backoff_sequence_is_capped() {
    let backoff = Backoff {
        initial: Duration::from_millis(100),
        factor: 2.0,
        max: Duration::from_millis(300),
        max_attempts: 5,
    };
    assert_eq!(backoff.delay(0), Some(Duration::from_millis(100)));
    assert_eq!(backoff.delay(1), Some(Duration::from_millis(200)));
    assert_eq!(backoff.delay(2), Some(Duration::from_millis(300)));
    assert_eq!(backoff.delay(3), Some(Duration::from_millis(300)));
    assert_eq!(backoff.delay(5), None);
}

#[test]
fn reconnect_disabled_returns_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        drop(stream);
    });
    let mut client = ReconnectingClient::new(
        move || TcpStream::connect(addr),
        Backoff {
            max_attempts: 0,
            ..Backoff::default()
        },
    );
    client.connect().unwrap();
    let err = client.next_packet().unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotConnected);
    server.join().unwrap();
}
