use std::path::PathBuf;

use clap::Parser;
use wrm_dl::{download, Config};

#[derive(Parser)]
#[command(long_about = None)]
struct Args {
    /// Name of the crate to download documentation for.
    crate_name: String,

    /// Version of the crate (defaults to "latest").
    #[arg(short, long)]
    version: Option<String>,

    /// Root directory to save the documentation to (defaults to temp dir).
    #[arg(short, long)]
    root: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let client = reqwest::Client::new();
    let mut config = Config::default().crate_name(args.crate_name).client(client);

    if let Some(version) = args.version {
        config = config.version(version);
    }

    if let Some(root) = args.root {
        config = config.root(root);
    }

    let path = download(config).await?;

    println!(
        "Documentation downloaded successfully to {}",
        path.to_string_lossy()
    );

    Ok(())
}
