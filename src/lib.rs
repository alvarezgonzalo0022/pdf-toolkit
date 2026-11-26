pub mod cli;
pub mod compress;
pub mod merge;
pub mod utils;
mod image; // privado, solo lo usa compress

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
