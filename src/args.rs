use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version = "0.1", about = "A fast C++ editor like Neovim")]
pub struct CliArgs {
    pub file_path: Option<PathBuf>,
}
