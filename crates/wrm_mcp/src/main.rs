use mcp_server::{router::RouterService, ByteTransport, Server};
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("LOG").add_directive(Level::INFO.into()))
        .with_target(false)
        .with_line_number(true)
        .with_writer(std::io::stderr)
        .init();

    let server = Server::new(RouterService(wrm_mcp::Server));
    let transport = ByteTransport::new(tokio::io::stdin(), tokio::io::stdout());

    info!("Bookworm MCP server initialized.");
    if let Err(error) = server.run(transport).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
