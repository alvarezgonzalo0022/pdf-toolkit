use std::path::{Path, PathBuf};

use pdf_toolkit::utils::{default_output, human_size, parse_size};

#[test]
fn test_parse_size() {
    assert_eq!(parse_size("1048576").unwrap(), 1048576);
    assert_eq!(parse_size("900kb").unwrap(), 900 * 1024);
    assert_eq!(parse_size("1.5mb").unwrap(), (1.5 * 1024.0 * 1024.0) as u64);
    assert_eq!(parse_size("24mb").unwrap(), 24 * 1024 * 1024);
    assert_eq!(parse_size("1gb").unwrap(), 1024 * 1024 * 1024);
    assert_eq!(parse_size("100b").unwrap(), 100);
    assert!(parse_size("abc").is_err());
    assert!(parse_size("").is_err());
}

#[test]
fn test_human_size() {
    assert_eq!(human_size(0), "0.0 B");
    assert_eq!(human_size(1023), "1023.0 B");
    assert_eq!(human_size(1024), "1.0 KB");
    assert_eq!(human_size(1048576), "1.0 MB");
    assert_eq!(human_size(1073741824), "1.0 GB");
}

#[test]
fn test_default_output() {
    assert_eq!(
        default_output(Path::new("a.pdf"), "-merged.pdf"),
        PathBuf::from("a-merged.pdf")
    );
    assert_eq!(
        default_output(Path::new("a.b.pdf"), "-merged.pdf"),
        PathBuf::from("a.b-merged.pdf")
    );
    assert_eq!(
        default_output(Path::new("input.pdf"), "-compressed.pdf"),
        PathBuf::from("input-compressed.pdf")
    );
}
