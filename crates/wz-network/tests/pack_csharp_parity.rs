//! C# `WcClient.EncodePack`(Newtonsoft 설정) 결과와 비교한다.

use serde::Deserialize;
use wz_network::pack::Packet;

#[derive(Deserialize)]
struct Row {
    label: String,
    hex: String,
    json: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn packets_match_csharp() {
    let rows: Vec<Row> = serde_json::from_str(include_str!("packets.json")).unwrap();
    for row in rows {
        let packet = match row.label.as_str() {
            "heartbeat" => Packet::Heartbeat,
            "loginreq" => Packet::LoginReq {
                wc_id: "wc-01",
                nick_name: "테스터",
            },
            "loginresp" => Packet::LoginResp {
                session_id: "sess-42",
            },
            "sendchat" => Packet::SendChat {
                message: Some("안녕"),
                group: 0,
            },
            "sendchat-null" => Packet::SendChat {
                message: None,
                group: 0,
            },
            "cryptreq" => Packet::CryptReq {
                exponent: &[1, 2, 3],
                modulus: &[9, 8, 7, 6],
            },
            "cryptresp" => Packet::CryptResp {
                key_encrypted_s2c: &[1, 2],
                key_encrypted_c2s: &[3, 4],
            },
            other => panic!("unknown label {other}"),
        };
        assert_eq!(packet.json(), row.json, "{} json", row.label);
        assert_eq!(
            hex(&packet.encode().unwrap()),
            row.hex,
            "{} frame",
            row.label
        );
    }
}
