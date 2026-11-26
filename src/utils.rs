use std::path::{Path, PathBuf};

/// Format a byte count as a human-readable string.
pub fn human_size(n: i64) -> String {
    let mut v = n as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if v < 1024.0 {
            return format!("{v:.1} {unit}");
        }
        v /= 1024.0;
    }
    format!("{v:.1} TB")
}

/// Convert '24mb' / '1.5mb' / '900kb' / '1048576' to bytes.
pub fn parse_size(text: &str) -> std::result::Result<u64, String> {
    let t = text.trim().to_lowercase();
    let mut mult: f64 = 1.0;
    let mut rest: &str = t.as_str();
    for (suffix, m) in [
        ("kb", 1024.0_f64),
        ("mb", 1024.0 * 1024.0),
        ("gb", 1024.0 * 1024.0 * 1024.0),
        ("b", 1.0),
    ] {
        if rest.ends_with(suffix) {
            mult = m;
            rest = &rest[..rest.len() - suffix.len()];
            break;
        }
    }
    let value: f64 = rest
        .trim()
        .parse()
        .map_err(|_| format!("invalid size: {text:?}"))?;
    Ok((value * mult) as u64)
}

pub fn default_output(input: &Path, suffix: &str) -> PathBuf {
    let mut name = input
        .file_stem()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    input.with_file_name(name)
}
