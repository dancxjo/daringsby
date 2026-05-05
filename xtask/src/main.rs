use anyhow::{Context, Result, bail};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const DEFAULT_MODEL: &str = "large-v3";
const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin";
const DEFAULT_MODEL_PATH: &str = "models/whisper/ggml-large-v3.bin";
const DEFAULT_VOICE_EMBEDDING_MODEL_URL: &str =
    "https://github.com/mzdk100/voxudio/releases/download/model/speaker_embedding_extractor.onnx";
const DEFAULT_VOICE_EMBEDDING_MODEL_PATH: &str = "models/voice/speaker_embedding_extractor.onnx";

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("fetch") => fetch_asr_model(args.next())?,
        Some("fetch-asr-model") => fetch_asr_model(args.next())?,
        Some("fetch-voice-embedding-model") => fetch_voice_embedding_model(args.next())?,
        Some("help") | Some("--help") | Some("-h") | None => print_help(),
        Some(other) => bail!("unknown xtask command: {other}"),
    }
    Ok(())
}

fn print_help() {
    println!("xtask commands:");
    println!("  fetch [tiny.en|base.en|small.en|large-v3|URL]            # fetches local models");
    println!(
        "  fetch-asr-model [tiny.en|base.en|small.en|large-v3|URL]  # also fetches voice embeddings"
    );
    println!("  fetch-voice-embedding-model [URL]");
}

fn fetch_asr_model(choice: Option<String>) -> Result<()> {
    let choice = choice.unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let (url, path) = model_choice(&choice);
    fetch_model(&url, &path, "ASR", "WHISPER_MODEL")?;
    fetch_voice_embedding_model(None)
}

fn fetch_voice_embedding_model(choice: Option<String>) -> Result<()> {
    let (url, path) = voice_embedding_model_choice(choice.as_deref());
    fetch_model(&url, &path, "voice embedding", "VOICE_EMBEDDING_MODEL")
}

fn fetch_model(url: &str, path: &Path, label: &str, env_key: &str) -> Result<()> {
    if path.exists() {
        println!("{label} model already exists: {}", path.display());
        println!("{env_key}={}", path.display());
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let tmp = path.with_extension("bin.part");
    println!("Downloading {url}");
    println!("Writing {}", path.display());

    let mut response = reqwest::blocking::get(url)
        .with_context(|| format!("failed to request {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed for {url}"))?;
    let total = response.content_length();
    let mut out =
        File::create(&tmp).with_context(|| format!("failed to create {}", tmp.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut written = 0u64;

    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        written += n as u64;
        if let Some(total) = total {
            print!("\r{:.1}%", written as f64 * 100.0 / total as f64);
        } else {
            print!("\r{} bytes", written);
        }
        let _ = std::io::stdout().flush();
    }
    println!();
    drop(out);

    fs::rename(&tmp, &path)
        .with_context(|| format!("failed to move {} to {}", tmp.display(), path.display()))?;
    println!("Fetched {label} model: {}", path.display());
    println!("{env_key}={}", path.display());
    Ok(())
}

fn model_choice(choice: &str) -> (String, PathBuf) {
    if choice.starts_with("http://") || choice.starts_with("https://") {
        let filename = choice.rsplit('/').next().unwrap_or("ggml-base.en.bin");
        return (
            choice.to_string(),
            Path::new("models").join("whisper").join(filename),
        );
    }

    let file = match choice {
        "tiny.en" => "ggml-tiny.en.bin",
        "base.en" => "ggml-base.en.bin",
        "small.en" => "ggml-small.en.bin",
        "large-v3" => "ggml-large-v3.bin",
        other => other,
    };
    let url = if choice == DEFAULT_MODEL {
        DEFAULT_MODEL_URL.to_string()
    } else {
        format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}")
    };
    let path = if choice == DEFAULT_MODEL {
        PathBuf::from(DEFAULT_MODEL_PATH)
    } else {
        Path::new("models").join("whisper").join(file)
    };
    (url, path)
}

fn voice_embedding_model_choice(choice: Option<&str>) -> (String, PathBuf) {
    match choice {
        Some(choice) if choice.starts_with("http://") || choice.starts_with("https://") => {
            let filename = choice
                .rsplit('/')
                .next()
                .unwrap_or("speaker_embedding_extractor.onnx");
            (
                choice.to_string(),
                Path::new("models").join("voice").join(filename),
            )
        }
        Some(choice) if !choice.trim().is_empty() => (
            choice.to_string(),
            Path::new("models").join("voice").join(choice),
        ),
        _ => (
            DEFAULT_VOICE_EMBEDDING_MODEL_URL.to_string(),
            PathBuf::from(DEFAULT_VOICE_EMBEDDING_MODEL_PATH),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_embedding_model_defaults_to_discovered_path() {
        let (url, path) = voice_embedding_model_choice(None);

        assert_eq!(url, DEFAULT_VOICE_EMBEDDING_MODEL_URL);
        assert_eq!(path, PathBuf::from(DEFAULT_VOICE_EMBEDDING_MODEL_PATH));
    }

    #[test]
    fn voice_embedding_model_url_uses_voice_model_directory() {
        let (url, path) =
            voice_embedding_model_choice(Some("https://example.com/custom-speaker.onnx"));

        assert_eq!(url, "https://example.com/custom-speaker.onnx");
        assert_eq!(path, PathBuf::from("models/voice/custom-speaker.onnx"));
    }

    #[test]
    fn default_model_is_large_multilingual_whisper() {
        let (url, path) = model_choice(DEFAULT_MODEL);

        assert_eq!(
            url,
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"
        );
        assert_eq!(path, PathBuf::from("models/whisper/ggml-large-v3.bin"));
    }

    #[test]
    fn model_choice_keeps_existing_english_aliases() {
        let (url, path) = model_choice("base.en");

        assert_eq!(
            url,
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
        );
        assert_eq!(path, PathBuf::from("models/whisper/ggml-base.en.bin"));
    }
}
