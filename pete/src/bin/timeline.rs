use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{GraphSensationTimelineItem, Neo4jClient, model::localized_timestamp};
use std::collections::HashSet;
use tokio::time::{Duration as TokioDuration, MissedTickBehavior, interval};

#[derive(Parser)]
#[command(author, version, about = "Print a text timeline of sensations")]
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
    /// Inclusive start time, as RFC3339, e.g. 2026-05-07T12:00:00Z. Omit for the beginning of recorded history.
    #[arg(long)]
    from: Option<String>,
    /// Inclusive end time, as RFC3339, e.g. 2026-05-07T12:01:30Z.
    #[arg(long)]
    to: Option<String>,
    /// Maximum sensation items to print; 0 prints all items in the time window.
    #[arg(long, default_value_t = 0)]
    limit: usize,
    /// Keep polling and print newly observed sensations.
    #[arg(long)]
    follow: bool,
    /// Poll delay for --follow.
    #[arg(long, env = "TIMELINE_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
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
    if cli.follow {
        follow_timeline(&graph, &cli).await?;
        return Ok(());
    }

    let (from, to) = time_range(&cli)?;
    let items = graph
        .sensation_timeline(from, to, cli.limit)
        .await
        .context("failed to load impression timeline")?;

    print_timeline(from, to, &items);
    Ok(())
}

async fn follow_timeline(graph: &Neo4jClient, cli: &Cli) -> anyhow::Result<()> {
    let mut seen = HashSet::new();
    let mut ticker = interval(TokioDuration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;
        let (from, to) = time_range(cli)?;
        let items = graph
            .sensation_timeline(from, to, cli.limit)
            .await
            .context("failed to load impression timeline")?;
        let mut new_items = Vec::new();
        for item in items {
            if seen.insert(item.id.clone()) {
                new_items.push(item);
            }
        }
        if !new_items.is_empty() {
            let mut current_time = String::new();
            let mut current_texts = Vec::new();
            for item in new_items {
                let ts = timeline_timestamp(&item.occurred_at);
                if ts != current_time {
                    if !current_texts.is_empty() {
                        println!("[{}] {}", current_time, current_texts.join(" "));
                    }
                    current_time = ts;
                    current_texts.clear();
                }
                current_texts.push(item.text);
            }
            if !current_texts.is_empty() {
                println!("[{}] {}", current_time, current_texts.join(" "));
            }
        }
    }
}

fn time_range(cli: &Cli) -> anyhow::Result<(Option<DateTime<Utc>>, DateTime<Utc>)> {
    let to = match &cli.to {
        Some(value) => parse_time(value).context("invalid --to")?,
        None => Utc::now(),
    };
    let from = match &cli.from {
        Some(value) => Some(parse_time(value).context("invalid --from")?),
        None => None,
    };
    if let Some(from) = from.as_ref() {
        anyhow::ensure!(from <= &to, "--from must be earlier than or equal to --to");
    }
    Ok((from, to))
}

fn parse_time(value: &str) -> anyhow::Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn print_timeline(
    from: Option<DateTime<Utc>>,
    to: DateTime<Utc>,
    items: &[GraphSensationTimelineItem],
) {
    let from = from
        .map(localized_timestamp)
        .unwrap_or_else(|| "forever".to_string());
    println!("Sensation timeline {} to {}", from, localized_timestamp(to));
    if items.is_empty() {
        println!("(no sensations)");
        return;
    }
    let mut current_time = String::new();
    let mut current_texts = Vec::new();
    for item in items {
        let ts = timeline_timestamp(&item.occurred_at);
        if ts != current_time {
            if !current_texts.is_empty() {
                println!("[{}] {}", current_time, current_texts.join(" "));
            }
            current_time = ts;
            current_texts.clear();
        }
        current_texts.push(item.text.clone());
    }
    if !current_texts.is_empty() {
        println!("[{}] {}", current_time, current_texts.join(" "));
    }
}

fn timeline_timestamp(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| localized_timestamp(timestamp.with_timezone(&Utc)))
        .unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::timeline_timestamp;
    use chrono::Local;

    #[test]
    fn timeline_timestamp_prints_local_time_with_offset() {
        let timestamp = chrono::DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z").unwrap();
        let expected = timestamp
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S %:z")
            .to_string();

        assert_eq!(timeline_timestamp("2026-05-05T12:34:56Z"), expected);
    }
}
