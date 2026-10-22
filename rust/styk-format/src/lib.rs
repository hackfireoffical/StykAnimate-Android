//! Reader for StykAnimate native containers (`.styk`, `.stkz`, `.stykmov`).
//!
//! A port of the decoder in `backend/native_format.py`; the format is described in
//! `docs/native-file-format.md`. Versions 1 (zlib JSON) and 2 (JSON skeleton + binary sections)
//! are supported, as are original plain-JSON `.styk` files.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{Number, Value};
use sha2::{Digest, Sha256};
use std::io::Read;

const HEADER: usize = 56;
const SECTION: usize = 64;
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SKELETON_BYTES: u64 = 384 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StykError(pub String);

impl std::fmt::Display for StykError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StykError {}

fn fail<T>(message: &str) -> Result<T, StykError> {
    Err(StykError(message.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Styk,
    Stkz,
    Stykmov,
}

impl Kind {
    fn magic(self) -> &'static [u8; 4] {
        match self {
            Kind::Styk => b"STYK",
            Kind::Stkz => b"STKZ",
            Kind::Stykmov => b"STMV",
        }
    }
}

fn le_u16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn le_u64(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    u64::from_le_bytes(a)
}

fn to_usize(value: u64) -> Result<usize, StykError> {
    usize::try_from(value).or_else(|_| fail("The native document exceeds the uncompressed size limit."))
}

fn digest_matches(data: &[u8], digest: &[u8]) -> bool {
    Sha256::digest(data).as_slice() == digest
}

fn inflate(data: &[u8], raw_len: usize) -> Result<Vec<u8>, StykError> {
    let mut rest = data;
    let mut out = Vec::new();
    {
        let decoder = flate2::bufread::ZlibDecoder::new(&mut rest);
        decoder
            .take(raw_len as u64 + 1)
            .read_to_end(&mut out)
            .or_else(|_| fail("The StykAnimate file is damaged (invalid compressed data)."))?;
    }
    if out.len() != raw_len || !rest.is_empty() {
        return fail("The StykAnimate file is damaged (invalid payload length).");
    }
    Ok(out)
}

fn unzstd(data: &[u8], raw_len: usize) -> Result<Vec<u8>, StykError> {
    let mut decoder = ruzstd::StreamingDecoder::new(data)
        .or_else(|_| fail("The StykAnimate file is damaged (invalid compressed data)."))?;
    let mut out = Vec::new();
    (&mut decoder)
        .take(raw_len as u64 + 1)
        .read_to_end(&mut out)
        .or_else(|_| fail("The StykAnimate file is damaged (invalid compressed data)."))?;
    if out.len() != raw_len {
        return fail("The StykAnimate file is damaged (invalid payload length).");
    }
    Ok(out)
}

fn unpack(codec: u8, data: &[u8], raw_len: usize) -> Result<Vec<u8>, StykError> {
    match codec {
        0 => {
            if data.len() != raw_len {
                return fail("The StykAnimate file is damaged (invalid payload length).");
            }
            Ok(data.to_vec())
        }
        1 => inflate(data, raw_len),
        2 => unzstd(data, raw_len),
        _ => fail("Unsupported StykAnimate file version. Update the app to open this file."),
    }
}

/// Decode a container into its JSON document.
pub fn decode(data: &[u8], kind: Kind) -> Result<Value, StykError> {
    if data.len() as u64 > MAX_BYTES {
        return fail("The native file exceeds the compressed size limit.");
    }

    // Original plain-JSON .styk files stay readable.
    if kind == Kind::Styk {
        let mut text = data;
        if text.starts_with(&[0xEF, 0xBB, 0xBF]) {
            text = &text[3..];
        }
        while let Some((first, tail)) = text.split_first() {
            if matches!(first, b' ' | b'\t' | b'\r' | b'\n') {
                text = tail;
            } else {
                break;
            }
        }
        if text.first() == Some(&b'{') {
            return serde_json::from_slice(text).or_else(|_| fail("The StykAnimate file is not valid JSON."));
        }
    }

    if data.len() < HEADER {
        return fail("The StykAnimate file is truncated or has an invalid header.");
    }
    if &data[0..4] != kind.magic() {
        return fail("The StykAnimate file type does not match its extension.");
    }
    let version = data[4];
    let compression = data[5];
    let flags = le_u16(&data[6..8]);
    let size = le_u64(&data[8..16]);
    let packed = le_u64(&data[16..24]);
    let digest = &data[24..56];
    if !(version == 1 || version == 2) || compression > 2 || flags != 0 {
        return fail("Unsupported StykAnimate file version. Update the app to open this file.");
    }
    let limit = if version == 2 { MAX_SKELETON_BYTES } else { MAX_BYTES };
    if size == 0 || size > limit {
        return fail("The native document exceeds the uncompressed size limit.");
    }
    let size = to_usize(size)?;
    let packed = to_usize(packed)?;

    if version == 2 {
        return decode_v2(data, compression, size, packed, digest);
    }
    if compression != 1 {
        return fail("Unsupported StykAnimate file version. Update the app to open this file.");
    }
    if packed != data.len() - HEADER {
        return fail("The StykAnimate file is truncated or has trailing data.");
    }
    let payload = inflate(&data[HEADER..], size)?;
    if !digest_matches(&payload, digest) {
        return fail("The StykAnimate file is damaged (checksum mismatch).");
    }
    serde_json::from_slice(&payload).or_else(|_| fail("The StykAnimate file is not valid JSON."))
}

struct Record {
    kind: u8,
    codec: u8,
    raw: usize,
    stored: usize,
    digest: Vec<u8>,
    pos: usize,
}

fn decode_v2(data: &[u8], compression: u8, size: usize, packed: usize, digest: &[u8]) -> Result<Value, StykError> {
    if data.len() < HEADER + 4 {
        return fail("The StykAnimate file is truncated or has an invalid header.");
    }
    let count = le_u32(&data[HEADER..HEADER + 4]) as usize;
    let skeleton_end = (HEADER + 4)
        .checked_add(packed)
        .or_else(|| None)
        .ok_or_else(|| StykError("The StykAnimate file is truncated or has an invalid header.".into()))?;
    if count > 64 || skeleton_end + count * SECTION > data.len() {
        return fail("The StykAnimate file is truncated or has an invalid header.");
    }

    let mut pos = skeleton_end + count * SECTION;
    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        let offset = skeleton_end + index * SECTION;
        let rec = &data[offset..offset + SECTION];
        let (kind, codec) = (rec[0], rec[1]);
        let (reserved, pad) = (le_u16(&rec[2..4]), le_u32(&rec[4..8]));
        let (raw, stored) = (le_u64(&rec[8..16]), le_u64(&rec[16..24]));
        if kind != 1 && kind != 2 {
            return fail("The StykAnimate file has an unknown section kind.");
        }
        if codec > 2 || reserved != 0 || pad != 0 || rec[56..64].iter().any(|b| *b != 0) {
            return fail("Unsupported StykAnimate file version. Update the app to open this file.");
        }
        let (raw, stored) = (to_usize(raw)?, to_usize(stored)?);
        if raw as u64 > MAX_BYTES
            || pos.checked_add(stored).map_or(true, |end| end > data.len())
            || (codec == 0 && raw != stored)
        {
            return fail("The StykAnimate file is truncated or has an invalid section table.");
        }
        records.push(Record { kind, codec, raw, stored, digest: rec[24..56].to_vec(), pos });
        pos += stored;
    }
    if pos != data.len() {
        return fail("The StykAnimate file is truncated or has trailing data.");
    }

    let skeleton = unpack(compression, &data[HEADER + 4..skeleton_end], size)?;
    if !digest_matches(&skeleton, digest) {
        return fail("The StykAnimate file is damaged (checksum mismatch).");
    }
    let mut sections: Vec<(u8, Vec<u8>)> = Vec::with_capacity(count);
    for record in &records {
        let raw = unpack(record.codec, &data[record.pos..record.pos + record.stored], record.raw)?;
        if !digest_matches(&raw, &record.digest) {
            return fail("The StykAnimate file is damaged (checksum mismatch).");
        }
        sections.push((record.kind, raw));
    }

    let mut document: Value =
        serde_json::from_slice(&skeleton).or_else(|_| fail("The StykAnimate file is not valid JSON."))?;
    materialize(&mut document, &sections)?;
    Ok(document)
}

fn materialize(value: &mut Value, sections: &[(u8, Vec<u8>)]) -> Result<(), StykError> {
    let replacement = match &*value {
        Value::String(text) => resolve(text, sections)?,
        _ => None,
    };
    if let Some(resolved) = replacement {
        *value = resolved;
        return Ok(());
    }
    match value {
        Value::Array(items) => {
            for item in items {
                materialize(item, sections)?;
            }
        }
        Value::Object(map) => {
            for (_, item) in map.iter_mut() {
                materialize(item, sections)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn malformed<T>() -> Result<T, StykError> {
    fail("The StykAnimate file is damaged (malformed section marker).")
}

fn number(text: &str) -> Result<usize, StykError> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return malformed();
    }
    text.parse::<usize>().or_else(|_| malformed())
}

fn payload<'a>(sections: &'a [(u8, Vec<u8>)], index: usize, offset: usize, count: usize) -> Result<&'a [u8], StykError> {
    match sections.get(index) {
        Some((2, raw)) if offset.checked_add(count).map_or(false, |end| end <= raw.len()) => Ok(&raw[offset..offset + count]),
        _ => fail("The StykAnimate file is damaged (invalid payload section reference)."),
    }
}

fn resolve(text: &str, sections: &[(u8, Vec<u8>)]) -> Result<Option<Value>, StykError> {
    if let Some(rest) = text.strip_prefix("S2E:") {
        return Ok(Some(Value::String(rest.to_string())));
    }
    if let Some(rest) = text.strip_prefix("S2B:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() != 4 {
            return malformed();
        }
        let (section, offset, count) = (number(parts[1])?, number(parts[2])?, number(parts[3])?);
        return array(sections, parts[0], section, offset, count).map(Some);
    }
    if let Some(rest) = text.strip_prefix("S2S:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() != 3 {
            return malformed();
        }
        let bytes = payload(sections, number(parts[0])?, number(parts[1])?, number(parts[2])?)?;
        return Ok(Some(Value::String(STANDARD.encode(bytes))));
    }
    if let Some(rest) = text.strip_prefix("S2D:") {
        let parts: Vec<&str> = rest.splitn(4, ':').collect();
        if parts.len() != 4 {
            return malformed();
        }
        let bytes = payload(sections, number(parts[0])?, number(parts[1])?, number(parts[2])?)?;
        return Ok(Some(Value::String(format!("{}{}", parts[3], STANDARD.encode(bytes)))));
    }
    Ok(None)
}

fn array(sections: &[(u8, Vec<u8>)], kind: &str, section: usize, offset: usize, count: usize) -> Result<Value, StykError> {
    let width = match kind {
        "u8" => 1,
        "u16" => 2,
        "u32" | "i32" | "f32" => 4,
        "f64" => 8,
        _ => return malformed(),
    };
    let raw = match sections.get(section) {
        Some((1, raw)) => raw,
        _ => return fail("The StykAnimate file is damaged (invalid array section reference)."),
    };
    let end = count.checked_mul(width).and_then(|bytes| offset.checked_add(bytes));
    match end {
        Some(end) if offset % width == 0 && end <= raw.len() => {
            let mut out = Vec::with_capacity(count);
            for chunk in raw[offset..end].chunks_exact(width) {
                let item = match kind {
                    "u8" => Value::from(chunk[0]),
                    "u16" => Value::from(le_u16(chunk)),
                    "u32" => Value::from(le_u32(chunk)),
                    "i32" => Value::from(le_u32(chunk) as i32),
                    "f32" => finite(f32::from_bits(le_u32(chunk)) as f64)?,
                    _ => finite(f64::from_bits(le_u64(chunk)))?,
                };
                out.push(item);
            }
            Ok(Value::Array(out))
        }
        _ => fail("The StykAnimate file is damaged (invalid array section reference)."),
    }
}

fn finite(value: f64) -> Result<Value, StykError> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| StykError("The StykAnimate document contains non-finite numbers.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use serde_json::json;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(3));
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn header(magic: &[u8; 4], version: u8, compression: u8, size: usize, packed: usize, raw: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(magic);
        out.push(version);
        out.push(compression);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(size as u64).to_le_bytes());
        out.extend_from_slice(&(packed as u64).to_le_bytes());
        out.extend_from_slice(Sha256::digest(raw).as_slice());
        out
    }

    fn v1(json_text: &str) -> Vec<u8> {
        let packed = zlib(json_text.as_bytes());
        let mut out = header(b"STYK", 1, 1, json_text.len(), packed.len(), json_text.as_bytes());
        out.extend_from_slice(&packed);
        out
    }

    fn record(kind: u8, raw: &[u8]) -> Vec<u8> {
        let mut out = vec![kind, 0, 0, 0, 0, 0, 0, 0];
        out.extend_from_slice(&(raw.len() as u64).to_le_bytes());
        out.extend_from_slice(&(raw.len() as u64).to_le_bytes());
        out.extend_from_slice(Sha256::digest(raw).as_slice());
        out.extend_from_slice(&[0u8; 8]);
        out
    }

    fn v2(skeleton: &str, sections: &[(u8, &[u8])]) -> Vec<u8> {
        let packed = zlib(skeleton.as_bytes());
        let mut out = header(b"STYK", 2, 1, skeleton.len(), packed.len(), skeleton.as_bytes());
        out.extend_from_slice(&(sections.len() as u32).to_le_bytes());
        out.extend_from_slice(&packed);
        for (kind, raw) in sections {
            out.extend_from_slice(&record(*kind, raw));
        }
        for (_, raw) in sections {
            out.extend_from_slice(raw);
        }
        out
    }

    #[test]
    fn reads_plain_json() {
        let value = decode(b"\xEF\xBB\xBF  {\"a\":1}", Kind::Styk).unwrap();
        assert_eq!(value, json!({"a": 1}));
    }

    #[test]
    fn reads_version_1() {
        let value = decode(&v1(r#"{"frames":24,"name":"walk"}"#), Kind::Styk).unwrap();
        assert_eq!(value, json!({"frames": 24, "name": "walk"}));
    }

    #[test]
    fn reads_version_2_markers() {
        let skeleton = r#"{"xs":"S2B:u8:0:0:3","fs":"S2B:f32:0:4:2","blob":"S2S:1:0:3","img":"S2D:1:0:3:data:image/png;base64,","esc":"S2E:S2B:literal","plain":"hello"}"#;
        let mut arrays = vec![1u8, 2, 3, 0];
        arrays.extend_from_slice(&0.5f32.to_le_bytes());
        arrays.extend_from_slice(&(-2.0f32).to_le_bytes());
        let value = decode(&v2(skeleton, &[(1, &arrays), (2, b"abc")]), Kind::Styk).unwrap();
        assert_eq!(value["xs"], json!([1, 2, 3]));
        assert_eq!(value["fs"], json!([0.5, -2.0]));
        assert_eq!(value["blob"], json!("YWJj"));
        assert_eq!(value["img"], json!("data:image/png;base64,YWJj"));
        assert_eq!(value["esc"], json!("S2B:literal"));
        assert_eq!(value["plain"], json!("hello"));
    }

    #[test]
    fn rejects_wrong_kind_and_corruption() {
        let file = v1(r#"{"a":1}"#);
        assert!(decode(&file, Kind::Stkz).is_err());

        let mut flipped = file.clone();
        flipped[30] ^= 0xFF;
        assert!(decode(&flipped, Kind::Styk).is_err());

        let truncated = &file[..file.len() - 3];
        assert!(decode(truncated, Kind::Styk).is_err());
    }

    #[test]
    fn rejects_bad_section_references() {
        let skeleton = r#"{"xs":"S2B:u8:0:0:9"}"#;
        assert!(decode(&v2(skeleton, &[(1, &[1u8, 2, 3])]), Kind::Styk).is_err());
        let misaligned = r#"{"xs":"S2B:u16:0:1:1"}"#;
        assert!(decode(&v2(misaligned, &[(1, &[1u8, 2, 3, 4])]), Kind::Styk).is_err());
    }
}
