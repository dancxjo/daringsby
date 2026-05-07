use std::path::PathBuf;

use clap::Parser;
use dotenvy::dotenv;
use pete::{
    EventBus, init_logging,
    movie::{default_movie_path, default_time_range, default_work_dir, parse_time, render_graph_movie},
};
use psyche::Neo4jClient;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Render a WebM movie and WebVTT captions from Pete's graph timeline"
)]
struct Cli {
    /// Neo4j bolt or HTTP URI.
    #[arg(long, env = "NEO4J_URI", default_value = "bolt://localhost:7687")]
    neo4j_uri: String,
    /// Neo4j username.
    #[arg(long, env = "NEO4J_USER", default_value = "neo4j")]
    neo4j_user: String,
    /// Neo4j password.
    #[arg(long, env = "NEO4J_PASS", default_value = "password")]
    neo4j_pass: String,
    /// Inclusive start time, as RFC3339, e.g. 2026-05-07T12:00:00Z.
    #[arg(long)]
    from: Option<String>,
    /// Inclusive end time, as RFC3339, e.g. 2026-05-07T12:01:30Z.
    #[arg(long)]
    to: Option<String>,
    /// Output movie path. Defaults to movies/pete-<from>-<to>.webm.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Temporary working directory for extracted frames.
    #[arg(long)]
    work_dir: Option<PathBuf>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let graph = Neo4jClient::new(
        cli.neo4j_uri.clone(),
        cli.neo4j_user.clone(),
        cli.neo4j_pass.clone(),
    );
    let from = cli.from.as_deref().map(parse_time).transpose()?;
    let to = cli.to.as_deref().map(parse_time).transpose()?;
    let (from, to) = default_time_range(&graph, from, to).await?;
    let out = cli.out.clone().unwrap_or_else(|| default_movie_path(from, to));
    let work_dir = cli
        .work_dir
        .clone()
        .unwrap_or_else(|| default_work_dir(&out));

    render_graph_movie(&graph, out.clone(), work_dir, from, to).await?;

    println!("movie: {}", out.display());
    println!("captions: {}", out.with_extension("vtt").display());
    Ok(())
}
