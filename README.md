# Rust Multi-Format Text Extractor

This Rust project extracts plain text from various document formats and saves each as a `.txt` file.

## Supported Formats

- EPUB  
- FB2  
- DOCX  
- HTML  
- RTF  

## Features

- Pure Rust; no external processes needed.  
- Recursively process directories.  
- Outputs UTF-8 `.txt` files next to original documents.  
- Lossless text extraction where possible (layout may be lost).  


## Notes
* DOCX/EPUB/FB2 extraction only captures text nodes; tables and images are ignored.
* RTF extraction is lossy; only plain text is preserved.

## Usage
```bash
dox2txt "path/to/your/dir" [-r]
```
-r - remove files after extraction
