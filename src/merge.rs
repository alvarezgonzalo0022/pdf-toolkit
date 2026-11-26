use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mupdf::pdf::{InsertPdfOptions, PdfDocument, PdfWriteOptions};

use crate::cli::MergeArgs;
use crate::utils::{default_output, human_size};
use crate::Result;

fn abspath(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

#[derive(Debug)]
struct EncryptedPdfError {
    path: PathBuf,
}

impl fmt::Display for EncryptedPdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "encrypted PDF (password required): {}",
            self.path.display()
        )
    }
}

impl std::error::Error for EncryptedPdfError {}

fn merge_docs(inputs: &[PathBuf], verbose: bool) -> Result<(Vec<u8>, usize)> {
    let mut doc = PdfDocument::new();
    let mut total = 0usize;

    for path in inputs {
        let data = fs::read(path)?;
        let src = PdfDocument::from_bytes(&data)?;
        if src.needs_password()? {
            return Err(Box::new(EncryptedPdfError { path: path.clone() }));
        }
        let pages = src.page_count()? as usize;
        doc.insert_pdf(&src, InsertPdfOptions::default())?;
        if verbose {
            println!("  + {} ({} pages)", path.display(), pages);
        }
        total += pages;
    }

    let mut options = PdfWriteOptions::default();
    options.set_garbage_level(4); // garbage=4
    options.set_compress(true); // deflate=True

    let mut out = Vec::new();
    doc.write_to_with_options(&mut out, options)?;
    Ok((out, total))
}

pub fn run_merge(args: MergeArgs) -> Result<u8> {
    for input in &args.inputs {
        if !input.is_file() {
            eprintln!("error: input not found: {}", input.display());
            return Ok(2);
        }
    }

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| default_output(&args.inputs[0], "-merged.pdf"));

    let out_abs = abspath(&output);
    for input in &args.inputs {
        if abspath(input) == out_abs {
            eprintln!(
                "error: output would overwrite an input: {}",
                output.display()
            );
            return Ok(2);
        }
    }

    println!("inputs: {} files", args.inputs.len());

    let (out, total) = match merge_docs(&args.inputs, args.verbose) {
        Ok(merged) => merged,
        Err(err) => {
            if err.downcast_ref::<EncryptedPdfError>().is_some() {
                eprintln!("error: {err}");
                return Ok(2);
            }
            return Err(err);
        }
    };

    fs::write(&output, out)?;

    let out_size = fs::metadata(&output)?.len();
    println!(
        "output: {}  ({}, {} pages)",
        output.display(),
        human_size(out_size as i64),
        total
    );
    Ok(0)
}
