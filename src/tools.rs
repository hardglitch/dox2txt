use std::borrow::Cow;
use anyhow::anyhow;
use chardetng::EncodingDetector;
use encoding_rs::UTF_8;

pub fn is_utf8(data: &[u8]) -> bool {
    // First: verify if file is already valid UTF-8
    let (_, _, utf8_errors) = UTF_8.decode(data);
    !utf8_errors
}
pub fn decode_bytes(data: &'_ [u8]) -> anyhow::Result<Cow<'_, str>> {

    // Otherwise, detect encoding
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);
    let enc = detector.guess(None, false);

    // Try decode text
    let (text, _, had_errors) = enc.decode(data);
    if had_errors {
        let enc_name = enc.name();
        return Err(anyhow!("decode errors with {}", enc_name))
    }
    Ok(text)
}
pub fn safe_decode_bytes(data: &'_ [u8]) -> anyhow::Result<Cow<'_, str>> {
    let doc =
        if data.starts_with(&[0xFF, 0xFE]) {
            // UTF-16 LE
            String::from_utf16_lossy(
                &data[2..]
                    .chunks(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>(),
            )
                .into()
        }
        else if data.starts_with(&[0xFE, 0xFF]) {
            // UTF-16 BE
            String::from_utf16_lossy(
                &data[2..]
                    .chunks(2)
                    .map(|b| u16::from_be_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>(),
            )
                .into()
        }
        else if is_utf8(data) {
            // UTF-8
            String::from_utf8_lossy(data)
        }
        else {
            // Otherwise, detect encoding with chardetng
            decode_bytes(data)?
        };

    Ok(doc)
}
// Converts all RTF \'xx escape sequences to real characters
pub fn decode_rtf_escapes(rtf: &str) -> anyhow::Result<String> {
    let mut bytes = Vec::with_capacity(rtf.len());
    let mut chars = rtf.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            // Check for RTF hex escape sequence
            if chars.peek() == Some(&'\'') {
                chars.next(); // skip the apostrophe
                let hex1 = chars.next();
                let hex2 = chars.next();
                if let (Some(h1), Some(h2)) = (hex1, hex2) {
                    // Convert hex to byte
                    if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                        bytes.push(byte);
                        continue;
                    }
                }
                // Invalid hex sequence: keep as literal
                bytes.push(b'\\');
                bytes.push(b'\'');
                if let Some(h1) = hex1 { bytes.push(h1 as u8); }
                if let Some(h2) = hex2 { bytes.push(h2 as u8); }
                continue;
            }
        }

        // Normal ASCII character: encode as UTF-8 bytes
        let mut buf = [0; 4];
        let s = c.encode_utf8(&mut buf);
        bytes.extend_from_slice(s.as_bytes());
    }

    // Convert all collected bytes into UTF-8 string safely
    let text = decode_bytes(&bytes)?;
    Ok(text.into_owned())
}
pub fn clean_invalid_xml_chars(input: &str) -> String {
    input
        .chars()
        .filter(|&c| match c {
            '\u{9}' | '\u{A}' | '\u{D}' => true, // allowed
            '\u{20}'..='\u{D7FF}' |
            '\u{E000}'..='\u{FFFD}' |
            '\u{10000}'..='\u{10FFFF}' => true,
            _ => false,
        })
        .collect()
}
pub fn remove_dtd(xml: &str) -> String {
    if xml.contains("<!DOCTYPE") {
        xml.lines()
            .filter(|line| !line.trim_start().starts_with("<!DOCTYPE"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        xml.to_string()
    }
}
