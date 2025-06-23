use lingproc::OllamaProvider;

/// Build an [`OllamaProvider`] from command line arguments.
///
/// This helper simply forwards the provided `host` and `model` strings to
/// [`OllamaProvider::new`]. Use it to create providers from CLI or
/// environment variables. The function returns an error if the host URL is
/// invalid or if the given model is not available on the Ollama server.
///
/// ```
/// use pete::ollama_provider_from_args;
///
/// let provider = ollama_provider_from_args("http://localhost:11434", "mistral")
///     .expect("valid provider");
/// ```
///
/// # Errors
///
/// Propagates any error returned by [`OllamaProvider::new`], such as an invalid
/// URL or missing model.
pub fn ollama_provider_from_args(host: &str, model: &str) -> anyhow::Result<OllamaProvider> {
    Ok(OllamaProvider::new_with_defaults(Some(host), Some(model))?)
}
