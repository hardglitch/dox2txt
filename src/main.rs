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
            let text = convert_file(entry.path())?;
            let text = text.trim();
            if !text.is_empty() {
                let out = entry.path().with_extension("txt");
                println!("-> {}", out.display());

                fs::write(out, text)?;

                if args.get(2) == Some(&"-r".to_owned()) &&
                   !ext.eq_ignore_ascii_case("txt")
                {
                    fs::remove_file(entry.path())?
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
                else { encode_to_utf8(path, data.as_bytes())? };

            let doc = Document::parse(&xml)?;

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
        else { encode_to_utf8(path, &data)? };

    let doc = Document::parse(&xml)?;

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

    let rtf =
        if is_utf8(&data) { String::from_utf8_lossy(&data) }
        else { encode_to_utf8(path, &data)? };

    let text = RtfDocument::try_from(rtf.as_ref())
        .map(|d| d.get_text())
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(text)
}

// ---------- HTML | HTM ----------
fn extract_html(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let html =
        if is_utf8(&data) { String::from_utf8_lossy(&data) }
        else { encode_to_utf8(path, &data)? };

    let document = Html::parse_document(&html);
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
        else { encode_to_utf8(path, &data)? };

    Ok(txt.to_string())
}

fn is_utf8(data: &[u8]) -> bool {
    // First: verify if file is already valid UTF-8
    let (_, _, utf8_errors) = UTF_8.decode(data);
    !utf8_errors
}

fn encode_to_utf8<'a>(path: &Path, data: &'a [u8]) -> Result<Cow<'a, str>> {

    // Otherwise, detect encoding
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);
    let enc = detector.guess(None, false);

    // Try decode text
    let (text, _, had_errors) = enc.decode(data);
    if had_errors {
        let enc_name = enc.name();
        return Err(anyhow!("{}: decode errors with {}", path.display(), enc_name))
    }
    Ok(text)
}
