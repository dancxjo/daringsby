use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, bail};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{GraphMovieImageFrame, GraphMovieSpeechSegment, Neo4jClient};

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
    let (from, to) = time_range(&graph, &cli).await?;
    let out = cli
        .out
        .clone()
        .unwrap_or_else(|| default_movie_path(from, to));
    let captions = out.with_extension("vtt");
    let work_dir = cli
        .work_dir
        .clone()
        .unwrap_or_else(|| default_work_dir(&out));

    let export = load_export(&graph, from, to).await?;
    fs::create_dir_all(out.parent().unwrap_or_else(|| Path::new(".")))
        .with_context(|| format!("failed to create output directory for {}", out.display()))?;
    fs::create_dir_all(&work_dir)
        .with_context(|| format!("failed to create work directory {}", work_dir.display()))?;

    write_captions(&captions, from, to, &export.speech)?;
    render_movie(&out, &work_dir, from, to, &export.images)?;

    println!("movie: {}", out.display());
    println!("captions: {}", captions.display());
    Ok(())
}

struct MovieExport {
    images: Vec<GraphMovieImageFrame>,
    speech: Vec<GraphMovieSpeechSegment>,
}

async fn load_export(
    graph: &Neo4jClient,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> anyhow::Result<MovieExport> {
    let mut images = graph
        .movie_image_frames(from, to)
        .await
        .context("failed to load movie image frames")?;
    if let Some(prior) = graph
        .latest_movie_image_frame_before(from)
        .await
        .context("failed to load latest movie image frame before range")?
    {
        images.insert(0, prior);
    }
    images.sort_by(|left, right| {
        left.occurred_at
            .cmp(&right.occurred_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    images.dedup_by(|left, right| left.id == right.id);
    if images.is_empty() {
        bail!("no image frames available for movie export");
    }

    let speech = graph
        .movie_speech_segments(from, to)
        .await
        .context("failed to load movie speech segments")?;

    Ok(MovieExport { images, speech })
}

async fn time_range(
    graph: &Neo4jClient,
    cli: &Cli,
) -> anyhow::Result<(DateTime<Utc>, DateTime<Utc>)> {
    let to = match &cli.to {
        Some(value) => parse_time(value).context("invalid --to")?,
        None => {
            let latest = graph
                .latest_movie_timestamp()
                .await
                .context("failed to find latest movie timestamp")?
                .context("no image or speech timestamps are available for movie export")?;
            parse_time(&latest).context("latest movie timestamp was invalid")?
        }
    };
    let from = match &cli.from {
        Some(value) => parse_time(value).context("invalid --from")?,
        None => to - Duration::seconds(90),
    };
    anyhow::ensure!(from <= to, "--from must be earlier than or equal to --to");
    Ok((from, to))
}

fn render_movie(
    out: &Path,
    work_dir: &Path,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    images: &[GraphMovieImageFrame],
) -> anyhow::Result<()> {
    let frames_dir = work_dir.join("frames");
    if frames_dir.exists() {
        fs::remove_dir_all(&frames_dir)
            .with_context(|| format!("failed to clean {}", frames_dir.display()))?;
    }
    fs::create_dir_all(&frames_dir)
        .with_context(|| format!("failed to create {}", frames_dir.display()))?;

    let mut render_frames = Vec::new();
    for (index, frame) in images.iter().enumerate() {
        let path = frames_dir.join(format!(
            "frame-{index:05}.{}",
            image_extension(&frame.image.mime)
        ));
        let bytes = BASE64_STANDARD
            .decode(frame.image.base64.trim().as_bytes())
            .with_context(|| format!("failed to decode image {}", frame.id))?;
        fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
        render_frames.push((path, parse_time(&frame.occurred_at)?));
    }

    let concat_path = work_dir.join("frames.concat");
    write_concat_file(&concat_path, from, to, &render_frames)?;
    run_ffmpeg(&concat_path, out)
}

fn write_concat_file(
    path: &Path,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    frames: &[(PathBuf, DateTime<Utc>)],
) -> anyhow::Result<()> {
    let mut file = fs::File::create(path)
        .with_context(|| format!("failed to create concat file {}", path.display()))?;
    for (index, (frame_path, timestamp)) in frames.iter().enumerate() {
        let start = if index == 0 {
            from
        } else {
            (*timestamp).max(from)
        };
        let end = frames
            .get(index + 1)
            .map(|(_, next_at)| (*next_at).min(to))
            .unwrap_or(to);
        let duration = (end - start).num_milliseconds();
        if duration <= 0 {
            continue;
        }
        writeln!(file, "file '{}'", ffmpeg_path(frame_path)?)?;
        writeln!(file, "duration {:.3}", duration as f64 / 1000.0)?;
    }
    if let Some((last_path, _)) = frames.last() {
        writeln!(file, "file '{}'", ffmpeg_path(last_path)?)?;
    }
    Ok(())
}

fn run_ffmpeg(concat_path: &Path, out: &Path) -> anyhow::Result<()> {
    let status = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(concat_path)
        .arg("-vf")
        .arg("scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:black,format=yuv420p")
        .arg("-r")
        .arg("30")
        .arg("-c:v")
        .arg("libvpx-vp9")
        .arg("-b:v")
        .arg("0")
        .arg("-crf")
        .arg("32")
        .arg(out)
        .status()
        .context("failed to start ffmpeg; install ffmpeg to render movies")?;
    anyhow::ensure!(status.success(), "ffmpeg exited with status {status}");
    Ok(())
}

fn write_captions(
    path: &Path,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    speech: &[GraphMovieSpeechSegment],
) -> anyhow::Result<()> {
    let mut file = fs::File::create(path)
        .with_context(|| format!("failed to create captions file {}", path.display()))?;
    writeln!(file, "WEBVTT\n")?;
    for segment in speech {
        let start = parse_time(&segment.occurred_at)?;
        let end = segment_end(segment, start)?;
        let cue_start = start.max(from);
        let cue_end = end.min(to);
        if cue_end <= cue_start {
            continue;
        }
        writeln!(
            file,
            "{} --> {}",
            vtt_timestamp(cue_start - from),
            vtt_timestamp(cue_end - from)
        )?;
        writeln!(file, "{}\n", vtt_text(&segment.text))?;
    }
    Ok(())
}

fn segment_end(
    segment: &GraphMovieSpeechSegment,
    start: DateTime<Utc>,
) -> anyhow::Result<DateTime<Utc>> {
    if let Some(ended_at) = &segment.ended_at {
        return parse_time(ended_at);
    }
    let duration_ms = segment.end_ms.saturating_sub(segment.start_ms).max(1000);
    Ok(start + Duration::milliseconds(i64::from(duration_ms)))
}

fn parse_time(value: &str) -> anyhow::Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn default_movie_path(from: DateTime<Utc>, to: DateTime<Utc>) -> PathBuf {
    PathBuf::from("movies").join(format!(
        "pete-{}-{}.webm",
        compact_time(from),
        compact_time(to)
    ))
}

fn default_work_dir(out: &Path) -> PathBuf {
    out.parent().unwrap_or_else(|| Path::new(".")).join(format!(
        ".{}-work",
        out.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("pete-movie")
    ))
}

fn compact_time(value: DateTime<Utc>) -> String {
    value.format("%Y%m%dT%H%M%SZ").to_string()
}

fn image_extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => "jpg",
    }
}

fn ffmpeg_path(path: &Path) -> anyhow::Result<String> {
    let absolute = path
        .canonicalize()
        .with_context(|| format!("failed to resolve frame path {}", path.display()))?;
    Ok(absolute.to_string_lossy().replace('\'', "'\\''"))
}

fn vtt_text(text: &str) -> String {
    text.replace("-->", "->").trim().to_string()
}

fn vtt_timestamp(duration: Duration) -> String {
    let total_ms = duration.num_milliseconds().max(0);
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let seconds = (total_ms % 60_000) / 1000;
    let millis = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}
