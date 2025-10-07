use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use anyhow::anyhow;
use roxmltree::Document;
use rtf_parser::RtfDocument;
use scraper::{Html, Selector};
use zip::ZipArchive;
use crate::Format;
use crate::tools::*;

pub fn convert_file(path: &Path) -> anyhow::Result<String> {
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
pub fn extract_zipped(path: &Path, format: Format) -> anyhow::Result<String> {
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

            let cleaned_raw_xml = sanitize_xml(data.as_bytes())?;
            let doc = Document::parse(&cleaned_raw_xml)?;

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
pub fn extract_fb2(path: &Path) -> anyhow::Result<String> {
    let data = fs::read(path)?;
    let cleaned_raw_xml = sanitize_xml(&data)?;
    let doc = Document::parse(&cleaned_raw_xml)?;

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
pub fn extract_rtf(path: &Path) -> anyhow::Result<String> {
    let data = fs::read(path)?;
    let raw_rtf = safe_decode_bytes(&data)?;
    let cleaned_raw_rtf = raw_rtf.trim();

    // Decode RTF escape sequences like \'xx into actual bytes
    let decoded_rtf = decode_rtf_escapes(cleaned_raw_rtf)?;

    let text = RtfDocument::try_from(decoded_rtf)
        .map(|d| d.get_text())
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(text)
}

// ---------- HTML | HTM ----------
pub fn extract_html(path: &Path) -> anyhow::Result<String> {
    let data = fs::read(path)?;
    let cleaned_raw_xml = sanitize_xml(&data)?;
    let doc = Html::parse_document(&cleaned_raw_xml);

    let selector = Selector::parse("body")
        .map_err(|e| anyhow!(e.to_string()))?;

    let mut buf = String::new();
    for el in doc.select(&selector) {
        buf.push_str(&el.text().collect::<Vec<_>>().join(" "));
    }

    Ok(buf)
}

// ---------- TXT ----------
pub fn convert_to_utf8(path: &Path) -> anyhow::Result<String> {
    let data = fs::read(path)?;
    let txt =
        if is_utf8(&data) { String::from_utf8_lossy(&data) }
        else { decode_bytes(&data)? };

    Ok(txt.trim().to_string())
}
