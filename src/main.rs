use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use anyhow::{anyhow, Result};

// -------- EPUB/FB2/DOCX ----------
use zip::ZipArchive;
use roxmltree::Document;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

// -------- RTF ----------
use rtf_parser::RtfDocument;

// -------- HTML/HTM ----------
use scraper::{Html, Selector};

// -------- TXT ----------
use chardetng::EncodingDetector;
use encoding_rs::UTF_8;

#[derive(PartialEq)]
enum Format {
    Docx,
    Epub,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path/to/your/folder> [-r]", args[0]);
        std::process::exit(1);
    }

    let sup_ext = ["epub", "fb2", "docx", "rtf", "html", "htm", "txt"];

    let dir = PathBuf::from_str(&args[1])?;
    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() &&
           let Some(ext) = entry.path().extension() &&
           sup_ext.contains(&ext.to_ascii_lowercase().to_str().unwrap_or_default())
        {
            let out = entry.path().with_extension("txt");

            match convert_file(entry.path()) {
                Ok(text) => {
                    let text = text.trim();

                    if !text.is_empty() {
                        println!("-> {}", out.display());

                        fs::write(out, text)?;

                        if args.get(2) == Some(&"-r".to_owned()) &&
                            !ext.eq_ignore_ascii_case("txt")
                        {
                            fs::remove_file(entry.path())?
                        }

                    }
                }
                Err(e) => {
                    println!("xxx {} - {e}", out.display());
                }
            }
        }
    }

    Ok(())
}

fn convert_file(path: &Path) -> Result<String> {
    let ext = path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();

    match ext.as_str() {
        "epub"  => extract_zipped(path, Format::Epub),
        "fb2"   => extract_fb2(path),
        "docx"  => extract_zipped(path, Format::Docx),
        "rtf"   => extract_rtf(path),
        "html" | "htm" => extract_html(path),
        "txt" => convert_to_utf8(path),
        _ => anyhow::bail!("unsupported extension"),
    }
}

// ---------- EPUB/DOCX ----------
fn extract_zipped(path: &Path, format: Format) -> Result<String> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut buf = String::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        if format == Format::Epub && (f.name().ends_with(".xhtml") || f.name().ends_with(".html"))
                 ||
           format == Format::Docx && f.name() == "word/document.xml"
        {
            let mut data = String::new();
            f.read_to_string(&mut data)?;

            let xml =
                if is_utf8(data.as_bytes()) { String::from_utf8_lossy(data.as_bytes()) }
                else { decode_bytes(data.as_bytes())? };

            let doc = Document::parse(xml.trim())?;

            for n in doc.descendants().filter(|n| n.is_text()) {
                if let Some(t) = n.text() && !t.is_empty() {
                    buf.push_str(t);
                    buf.push(' ');
                }
            }
        }
    }
    Ok(buf)
}

// ---------- FB2 ----------
fn extract_fb2(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let xml =
        if is_utf8(&data) { String::from_utf8_lossy(&data) }
        else { decode_bytes(&data)? };

    let doc = Document::parse(xml.trim())?;

    let mut buf = String::new();
    for n in doc.descendants().filter(|n| n.is_text()) {
        if let Some(t) = n.text() && !t.is_empty() {
            buf.push_str(t);
            buf.push(' ');
        }
    }

    Ok(buf)
}

// ---------- RTF ----------
fn extract_rtf(path: &Path) -> Result<String> {
    let data = fs::read(path)?;

    // Detect BOM for UTF-16
    let rtf = if data.starts_with(&[0xFF, 0xFE]) {
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
    else if is_utf8(&data) {
        // UTF-8
        String::from_utf8_lossy(&data)
    }
    else {
        // Otherwise, detect encoding with chardetng
        decode_bytes(&data)?
    };

    let cleaned_rtf = rtf.trim().trim_matches('\0');

    // Decode RTF escape sequences like \'xx into actual bytes
    let decoded_rtf = decode_rtf_escapes(cleaned_rtf)?;

    let text = RtfDocument::try_from(decoded_rtf)
        .map(|d| d.get_text())
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(text)
}

// ---------- HTML | HTM ----------
fn extract_html(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let html =
        if is_utf8(&data) { String::from_utf8_lossy(&data) }
        else { decode_bytes(&data)? };

    let document = Html::parse_document(html.trim());
    let selector = Selector::parse("body")
        .map_err(|e| anyhow!(e.to_string()))?;

    let mut buf = String::new();
    for el in document.select(&selector) {
        buf.push_str(&el.text().collect::<Vec<_>>().join(" "));
    }

    Ok(buf)
}

// ---------- TXT ----------
fn convert_to_utf8(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let txt =
        if is_utf8(&data) { String::from_utf8_lossy(&data) }
        else { decode_bytes(&data)? };

    Ok(txt.trim().to_string())
}

fn is_utf8(data: &[u8]) -> bool {
    // First: verify if file is already valid UTF-8
    let (_, _, utf8_errors) = UTF_8.decode(data);
    !utf8_errors
}

fn decode_bytes(data: &'_ [u8]) -> Result<Cow<'_, str>> {

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

// Converts all RTF \'xx escape sequences to real characters
fn decode_rtf_escapes(rtf: &str) -> Result<String> {
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



#[cfg(test)]
#[ignore]
#[test]
fn test() {
    let p = Path::new("./1.rtf");
    let _ = extract_rtf(p);
}
