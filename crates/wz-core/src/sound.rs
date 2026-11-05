//! `Sound_DX8` 디코딩 — `WAVEFORMATEX` 복호화와 WAV/MP3 추출.
//!
//! 원본 `Wz_Sound.ExtractSound` / `Wz_Image.TryDecryptWaveFormatEx` 대응.
//! 재생은 플랫폼 계층(예: macOS `afplay`)이 맡고, 여기서는 파일 바이트를
//! 정확히 만든다.

use std::sync::Arc;

use wz_error::{Result, WzFormatError};
use wz_pool::ByteSource;

use crate::crypto::aes::AesKeyStream;
use crate::crypto::Decryptor;
use crate::image::SoundRef;

/// `WAVEFORMATEX` (18바이트 기본 + cbSize 확장).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveFormat {
    pub format_tag: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub cb_size: u16,
}

/// 사운드 종류 (`Wz_SoundType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCodec {
    Pcm,
    Mp3,
    Binary,
}

/// 추출된 재생 가능한 바이트.
#[derive(Debug, Clone)]
pub struct ExtractedSound {
    pub codec: SoundCodec,
    pub extension: &'static str,
    pub bytes: Vec<u8>,
    pub format: Option<WaveFormat>,
}

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_MPEGLAYER3: u16 = 0x0055;
const WAVEFORMATEX_SIZE: usize = 18;
const MPEGLAYER3WAVEFORMAT_SIZE: usize = 30;

fn parse_wave_format(bytes: &[u8]) -> Option<WaveFormat> {
    if bytes.len() < WAVEFORMATEX_SIZE {
        return None;
    }
    let u16_at = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]);
    let u32_at =
        |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let fmt = WaveFormat {
        format_tag: u16_at(0),
        channels: u16_at(2),
        samples_per_sec: u32_at(4),
        avg_bytes_per_sec: u32_at(8),
        block_align: u16_at(12),
        bits_per_sample: u16_at(14),
        cb_size: u16_at(16),
    };
    if fmt.channels == 0 || fmt.samples_per_sec == 0 {
        return None;
    }
    let valid = match fmt.format_tag {
        WAVE_FORMAT_PCM => {
            fmt.bits_per_sample > 0
                && fmt.block_align > 0
                && (bytes.len() == fmt.cb_size as usize + WAVEFORMATEX_SIZE
                    || bytes.len() >= WAVEFORMATEX_SIZE)
        }
        WAVE_FORMAT_MPEGLAYER3 => {
            bytes.len() == MPEGLAYER3WAVEFORMAT_SIZE
                || bytes.len() == WAVEFORMATEX_SIZE
                || bytes.len() == fmt.cb_size as usize
        }
        _ => false,
    };
    valid.then_some(fmt)
}

fn decrypt_with(mut bytes: Vec<u8>, key: &AesKeyStream) -> Vec<u8> {
    key.decrypt_at(&mut bytes, 0);
    bytes
}

/// `WAVEFORMATEX` 복호화. 원본과 같이 평문(BMS) → KMS → GMS 순서로 시도한다.
pub fn decode_wave_format(fmt_ex: &[u8]) -> Option<WaveFormat> {
    if fmt_ex.is_empty() {
        return None;
    }
    if let Some(fmt) = parse_wave_format(fmt_ex) {
        return Some(fmt);
    }
    for key in [AesKeyStream::kms(), AesKeyStream::gms()] {
        let candidate = decrypt_with(fmt_ex.to_vec(), &key);
        if let Some(fmt) = parse_wave_format(&candidate) {
            return Some(fmt);
        }
    }
    None
}

fn pcm_to_wav(data: &[u8], fmt: &WaveFormat) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((data.len() + 36) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&fmt.format_tag.to_le_bytes());
    out.extend_from_slice(&fmt.channels.to_le_bytes());
    out.extend_from_slice(&fmt.samples_per_sec.to_le_bytes());
    out.extend_from_slice(&fmt.avg_bytes_per_sec.to_le_bytes());
    out.extend_from_slice(&fmt.block_align.to_le_bytes());
    out.extend_from_slice(&fmt.bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// 사운드 데이터 바이트를 소스에서 읽어 재생 가능한 형식으로 만든다.
pub fn extract_sound(src: &Arc<dyn ByteSource>, sound: &SoundRef) -> Result<ExtractedSound> {
    let mut data = vec![0u8; sound.data_len as usize];
    src.read_at(sound.offset, &mut data)?;
    let fmt = decode_wave_format(&sound.fmt_ex);

    let codec = match (fmt.as_ref(), sound) {
        (Some(f), _) if f.format_tag == WAVE_FORMAT_PCM => SoundCodec::Pcm,
        (Some(f), _) if f.format_tag == WAVE_FORMAT_MPEGLAYER3 => SoundCodec::Mp3,
        _ if sound.duration == 1000
            && fmt
                .as_ref()
                .map(|f| f.samples_per_sec == sound.data_len)
                .unwrap_or(false) =>
        {
            SoundCodec::Binary
        }
        _ => {
            // 포맷을 모르면 원본과 같이 미디어 서브타입을 본다.
            if sound.sub_type == MEDIASUBTYPE_MPEG1_AUDIO {
                SoundCodec::Mp3
            } else if sound.sub_type == MEDIASUBTYPE_WAVE {
                SoundCodec::Binary
            } else {
                return Err(WzFormatError::Malformed {
                    what: "unknown sound format",
                    offset: sound.offset,
                }
                .into());
            }
        }
    };

    let (bytes, extension) = match codec {
        SoundCodec::Pcm => {
            let fmt = fmt.clone().ok_or(WzFormatError::Malformed {
                what: "pcm without wave format",
                offset: sound.offset,
            })?;
            (pcm_to_wav(&data, &fmt), "wav")
        }
        SoundCodec::Mp3 => (data, "mp3"),
        SoundCodec::Binary => (data, "bin"),
    };
    Ok(ExtractedSound {
        codec,
        extension,
        bytes,
        format: fmt,
    })
}

// MEDIASUBTYPE_MPEG1Audio / MEDIASUBTYPE_WAVE GUID 바이트.
const MEDIASUBTYPE_MPEG1_AUDIO: [u8; 16] = [
    0x55, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
const MEDIASUBTYPE_WAVE: [u8; 16] = [
    0x57, 0x41, 0x56, 0x45, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_pcm_format() {
        let bytes = [
            1, 0, 2, 0, 0x44, 0xAC, 0, 0, 0x10, 0xB1, 0x02, 0, 4, 0, 16, 0, 0, 0,
        ];
        let fmt = decode_wave_format(&bytes).expect("pcm");
        assert_eq!(fmt.format_tag, 1);
        assert_eq!(fmt.channels, 2);
        assert_eq!(fmt.samples_per_sec, 44100);
        assert_eq!(fmt.block_align, 4);
        assert_eq!(fmt.bits_per_sample, 16);
    }

    #[test]
    fn parses_kms_encrypted_format() {
        let plain = [
            1, 0, 1, 0, 0x40, 0x1F, 0, 0, 0x40, 0x1F, 0, 0, 1, 0, 8, 0, 0, 0,
        ];
        let mut enc = plain.to_vec();
        // 암호화는 복호화와 같은 XOR 이다 (AES 키스트림).
        let stream = AesKeyStream::kms();
        let mut ks = vec![0u8; enc.len()];
        stream.decrypt_at(&mut ks, 0);
        for (a, b) in enc.iter_mut().zip(ks) {
            *a ^= b;
        }
        let fmt = decode_wave_format(&enc).expect("kms");
        assert_eq!(fmt.samples_per_sec, 8000);
        assert_eq!(fmt.bits_per_sample, 8);
    }

    #[test]
    fn extracts_real_bgm() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let path = root.join("Sound").join("Sound_000.wz");
        if !path.exists() {
            return;
        }
        let archive = crate::WzArchive::open(&path).unwrap();
        let tree = archive.read_tree().unwrap();
        let entry = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("Bgm00.img"))
            .expect("Bgm00.img");
        let node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        let sound = node
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == "DragonDream")
            .expect("DragonDream");
        let crate::image::ImageValue::Sound(sound) = sound.value.as_ref().unwrap() else {
            panic!("Sound 값이 아니다");
        };
        assert_eq!(sound.data_len, 1_261_192);
        assert_eq!(sound.duration, 126_119);
        let extracted = extract_sound(archive.source(), sound).expect("extract");
        assert_eq!(extracted.codec, SoundCodec::Mp3);
        assert_eq!(extracted.extension, "mp3");
        assert_eq!(extracted.bytes.len(), sound.data_len as usize);
        let fmt = extracted.format.expect("mp3 format");
        assert_eq!(fmt.channels, 2);
        assert_eq!(fmt.samples_per_sec, 22050);
    }

    #[test]
    fn builds_riff_header_for_pcm() {
        let sound = SoundRef {
            offset: 0,
            data_len: 4,
            duration: 0,
            sound_decl: 2,
            major_type: [0; 16],
            sub_type: [0; 16],
            fixed_size_samples: false,
            temporal_compression: false,
            format_type: [0; 16],
            fmt_ex: vec![
                1, 0, 1, 0, 0x40, 0x1F, 0, 0, 0x40, 0x1F, 0, 0, 1, 0, 8, 0, 0, 0,
            ],
        };
        let src: Arc<dyn ByteSource> = Arc::new(wz_pool::MemorySource::new(vec![1, 2, 3, 4]));
        let extracted = extract_sound(&src, &sound).unwrap();
        assert_eq!(extracted.codec, SoundCodec::Pcm);
        assert_eq!(&extracted.bytes[0..4], b"RIFF");
        assert_eq!(&extracted.bytes[44..], &[1, 2, 3, 4]);
    }
}
