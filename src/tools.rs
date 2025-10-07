use std::borrow::Cow;
use std::path::PathBuf;
use anyhow::anyhow;
use chardetng::EncodingDetector;
use encoding_rs::UTF_8;
use walkdir::WalkDir;

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
    if data.is_empty() {
        return Ok(Cow::Borrowed(""));
    }

    let doc =
        if data.len() >= 2 {
            match data {
                [0xFF, 0xFE, ..] => {
                    // UTF-16 LE
                    let u16s: Vec<u16> = data[2..]
                        .chunks_exact(2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .collect();
                    Cow::Owned(String::from_utf16_lossy(&u16s))
                }
                [0xFE, 0xFF, ..] => {
                    // UTF-16 BE
                    let u16s: Vec<u16> = data[2..]
                        .chunks_exact(2)
                        .map(|b| u16::from_be_bytes([b[0], b[1]]))
                        .collect();
                    Cow::Owned(String::from_utf16_lossy(&u16s))
                }
                _ => {
                    if is_utf8(data) { String::from_utf8_lossy(data) }
                    // Otherwise, detect encoding with chardetng
                    else { decode_bytes(data)? }
                }
            }
        }
        else { return Ok(Cow::Borrowed("")) };

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
        .filter(|&c|
            match c {
                '\u{9}' | '\u{A}' | '\u{D}' => true, // allowed
                '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}' => true,
                _ => false,
            })
        .collect()
}
pub fn is_dtd(xml: &str) -> bool {
    xml.contains("<!DOCTYPE")
}
pub fn remove_dtd(xml: &str) -> String {
    if let Some(start) = xml.find("<!DOCTYPE") {
        if let Some(end) = xml[start..].find('>') {
            // remove whole <!DOCTYPE ...> block
            let end = start + end + 1;
            let mut result = String::with_capacity(xml.len());
            result.push_str(&xml[..start]);
            result.push_str(&xml[end..]);
            result
        } else {
            // malformed DTD, just cut from <!DOCTYPE to end
            xml[..start].to_string()
        }
    } else {
        xml.to_string()
    }
}
pub fn fix_html_entities(s: &str) -> String {
    s.replace("&nbsp;", "\u{00A0}")
     .replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&amp;", "&")
     .replace("&quot;", "\"")
     .replace("&apos;", "'")
}
pub fn sanitize_xml(data: &[u8]) -> anyhow::Result<String> {
    let raw_xml = safe_decode_bytes(data)?;
    let raw_xml = raw_xml.trim();

    let cleaned_raw_xml =
        if is_dtd(raw_xml) {
            let xml = remove_dtd(raw_xml);
            let xml = fix_html_entities(&xml);
            clean_invalid_xml_chars(&xml)
        }
        else {
            clean_invalid_xml_chars(raw_xml)
        };

    Ok(cleaned_raw_xml)
}
pub fn sanitizer(path: &PathBuf) -> anyhow::Result<()> {

	// First remove empty files...
    for entry in WalkDir::new(path).into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        if std::fs::metadata(entry.path())?.len() == 0 {
            // File is empty, remove it
            std::fs::remove_file(entry.path())?;
        }
    }

	// ... and then remove empty dirs
    for entry in WalkDir::new(path).into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
    {
        if std::fs::read_dir(entry.path())?.next().is_none() {
            // Directory is empty, remove it
            std::fs::remove_dir(entry.path())?;
        }
    }

    Ok(())
}
