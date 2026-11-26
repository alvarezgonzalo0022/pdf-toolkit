use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::utils::parse_size;

#[derive(Parser)]
#[command(
    name = "pdf-toolkit",
    version,
    about = "A small multi-command toolkit for PDF files."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// reduce a PDF's file size
    Compress(CompressArgs),

    /// concatenate multiple PDFs into one
    Merge(MergeArgs),
}

#[derive(Args)]
pub struct CompressArgs {
    /// input PDF file
    pub input: PathBuf,

    /// output PDF file (default: <name>-compressed.pdf)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// target image resolution in DPI
    #[arg(long, default_value_t = 150)]
    pub dpi: u32,

    /// JPEG quality (1-100)
    #[arg(long, default_value_t = 70)]
    pub quality: u8,

    /// skip images already at or below this DPI
    #[arg(long, default_value_t = 72)]
    pub min_dpi: u32,

    /// convert images to grayscale
    #[arg(long)]
    pub grayscale: bool,

    /// target max size (e.g. 24mb); iteratively lowers quality/dpi to fit
    #[arg(long, value_parser = parse_size)]
    pub target: Option<u64>,

    /// print per-image details
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct MergeArgs {
    /// PDF files to merge, in order
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    /// output PDF file (default: <first-stem>-merged.pdf)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// print per-file details
    #[arg(long)]
    pub verbose: bool,
}
