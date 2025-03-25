use clap::Parser;
use wrm_index::{index, Config};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Index locally stored crate documentation into a SQLite database")]
struct Args {
    /// Path to the documentation directory to index.
    #[arg(index = 1)]
    source: PathBuf,

    /// Path to save the SQLite database to (defaults to ./index.sqlite).
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let output = args.output.unwrap_or_else(|| PathBuf::from("index.sqlite"));
    let config = Config::default().source(args.source).output(&output);

    index(config)?;
    println!("Documentation indexed successfully to {}", output.display());

    Ok(())
}
