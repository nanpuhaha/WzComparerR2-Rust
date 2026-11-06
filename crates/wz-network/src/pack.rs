//! `WzComparerR2.Network.Contracts` 패킷 직렬화.
//!
//! 원본 `WcClient.EncodePack`은 Newtonsoft.Json을
//! `TypeNameHandling.Objects` + `[JsonObject("id")]` 바인더 + `byte[]` →
//! Base64 컨버터로 직렬화한 뒤 UTF-8 바이트 앞에 uint16 LE 길이를 붙인다.
//! 여기서는 자주 쓰는 계약들을 같은 순서/문구로 만든다.

use crate::encode_frame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet<'a> {
    Heartbeat,
    LoginReq {
        wc_id: &'a str,
        nick_name: &'a str,
    },
    LoginResp {
        session_id: &'a str,
    },
    SendChat {
        message: Option<&'a str>,
        group: i32,
    },
    CryptReq {
        exponent: &'a [u8],
        modulus: &'a [u8],
    },
    CryptResp {
        key_encrypted_s2c: &'a [u8],
        key_encrypted_c2s: &'a [u8],
    },
}

fn json_str(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization")
}

fn json_bytes(value: &[u8]) -> String {
    json_str(&base64(value))
}

impl Packet<'_> {
    /// 본문 JSON. Newtonsoft의 프로퍼티 순서를 그대로 따른다.
    pub fn json(&self) -> String {
        match self {
            Packet::Heartbeat => r#"{"$type":"0"}"#.to_string(),
            Packet::LoginReq { wc_id, nick_name } => format!(
                r#"{{"$type":"3","WcID":{},"NickName":{}}}"#,
                json_str(wc_id),
                json_str(nick_name)
            ),
            Packet::LoginResp { session_id } => {
                format!(r#"{{"$type":"4","SessionID":{}}}"#, json_str(session_id))
            }
            Packet::SendChat { message, group } => format!(
                r#"{{"$type":"9","Message":{},"Group":{group}}}"#,
                match message {
                    Some(text) => json_str(text),
                    None => "null".to_string(),
                }
            ),
            Packet::CryptReq { exponent, modulus } => format!(
                r#"{{"$type":"1","Exponent":{},"Modulus":{}}}"#,
                json_bytes(exponent),
                json_bytes(modulus)
            ),
            Packet::CryptResp {
                key_encrypted_s2c,
                key_encrypted_c2s,
            } => format!(
                r#"{{"$type":"2","KeyEncryptedS2C":{},"KeyEncryptedC2S":{}}}"#,
                json_bytes(key_encrypted_s2c),
                json_bytes(key_encrypted_c2s)
            ),
        }
    }

    /// `EncodePack` — uint16 LE 길이 + UTF-8 JSON.
    pub fn encode(&self) -> Option<Vec<u8>> {
        encode_frame(self.json().as_bytes())
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_examples() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(&[1, 2, 3]), "AQID");
    }

    #[test]
    fn heartbeat_frame() {
        let frame = Packet::Heartbeat.encode().unwrap();
        assert_eq!(frame, b"\x0d\x00{\"$type\":\"0\"}");
    }
}
