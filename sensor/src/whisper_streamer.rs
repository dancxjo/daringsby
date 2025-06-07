use async_stream::stream;
use futures_util::stream::BoxStream;
use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Interim or final ASR result.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ASRStatus {
    Interim,
    Final,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ASRResult {
    pub transcript: String,
    pub status: ASRStatus,
}

/// Wrapper around whisper-rs that yields streaming transcripts.
pub struct WhisperStreamer {
    ctx: WhisperContext,
    params: FullParams<'static, 'static>,
}

impl WhisperStreamer {
    /// Load a Whisper model from the given path.
    pub fn new(model: &str) -> Self {
        let ctx = WhisperContext::new_with_params(model, WhisperContextParameters::default())
            .expect("load model");
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_translate(false);
        params.set_language(Some("en"));
        Self { ctx, params }
    }

    /// Produce a stream of ASR results for the provided audio samples.
    pub fn transcribe<'a>(&'a self, audio: &'a [f32]) -> BoxStream<'a, ASRResult> {
        let mut state = self.ctx.create_state().expect("state");
        let params = self.params.clone();
        Box::pin(stream! {
            if state.full(params, audio).is_ok() {
                let text = state.full_get_segment_text(0).unwrap_or_default();
                yield ASRResult { transcript: text, status: ASRStatus::Final };
            }
        })
    }
}
