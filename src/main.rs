mod tools;

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

use crate::tools::*;

#[derive(PartialEq)]
enum Format {
    Docx,
    Epub,
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <path/to/your/folder> [-r]", args[0]);
        std::process::exit(1);
    }

    let sup_ext = ["epub", "fb2", "docx", "rtf", "html", "htm", "txt"];
    let thrash_ext = ["djvu", "djv", "doc", "chm", "xls", "jpg", "jpeg", "gif", "png", "zip", "rar", "diz"];

    let dir = PathBuf::from_str(&args[1])?;
    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() &&
           let Some(ext) = entry.path().extension()
        {
        	let ext = ext.to_ascii_lowercase();
			let ext = ext.to_str().unwrap_or_default();
        	
        	if sup_ext.contains(&ext) {
	            let out = entry.path().with_extension("txt");
	
	            match convert_file(entry.path()) {
	                Ok(text) => {
	                    let text = text.trim();
	
	                    if !text.is_empty() {
	                        println!("-> {}", out.display());
	
	                        fs::write(out, text)?;
	
	                        if (args.get(2) == Some(&"-r".to_owned()) || args.get(3) == Some(&"-r".to_owned())) &&
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
        	else if thrash_ext.contains(&ext) &&
                   (args.get(2) == Some(&"-rt".to_owned()) || args.get(3) == Some(&"-rt".to_owned()))
            {
                fs::remove_file(entry.path())?
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

            let raw_xml = safe_decode_bytes(data.as_bytes())?;
            let cleaned_raw_xml = raw_xml.trim().trim_end_matches('\0');
            let cleaned_raw_xml = remove_dtd(cleaned_raw_xml);
            let cleaned_raw_xml = clean_invalid_xml_chars(&cleaned_raw_xml);
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
fn extract_fb2(path: &Path) -> Result<String> {
    let data = fs::read(path)?;

    let raw_xml = safe_decode_bytes(&data)?;
    let cleaned_raw_xml = raw_xml.trim().trim_end_matches('\0');
    let cleaned_raw_xml = remove_dtd(cleaned_raw_xml);
    let cleaned_raw_xml = clean_invalid_xml_chars(&cleaned_raw_xml);
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
fn extract_rtf(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let raw_rtf = safe_decode_bytes(&data)?;
    let cleaned_raw_rtf = raw_rtf.trim().trim_end_matches('\0');

    // Decode RTF escape sequences like \'xx into actual bytes
    let decoded_rtf = decode_rtf_escapes(cleaned_raw_rtf)?;

    let text = RtfDocument::try_from(decoded_rtf)
        .map(|d| d.get_text())
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(text)
}

// ---------- HTML | HTM ----------
fn extract_html(path: &Path) -> Result<String> {
    let data = fs::read(path)?;

    let raw_xml = safe_decode_bytes(&data)?;
    let cleaned_raw_xml = raw_xml.trim().trim_end_matches('\0');
    let cleaned_raw_xml = clean_invalid_xml_chars(cleaned_raw_xml);
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
fn convert_to_utf8(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let txt =
        if is_utf8(&data) { String::from_utf8_lossy(&data) }
        else { decode_bytes(&data)? };

    Ok(txt.trim().to_string())
}
