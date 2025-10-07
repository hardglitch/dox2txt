use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;
use crate::core::convert_file;

pub fn main_logic(dir: &PathBuf, args: &[String]) -> anyhow::Result<()> {
    let sup_ext = ["epub", "fb2", "docx", "rtf", "html", "htm", "txt"];
    let thrash_ext = ["djvu", "djv", "doc", "chm", "xls", "jpg", "jpeg", "gif", "png", "zip", "rar", "diz"];

    for entry in WalkDir::new(dir).into_iter().filter_map(anyhow::Result::ok) {
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