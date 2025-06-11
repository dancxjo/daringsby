//! Launches the Pete web server exposing log and chat events over WebSockets.
use anyhow::Result;
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    let bus = Arc::new(psyche::bus::EventBus::new());
    psyche::logging::init(bus.clone())?;

    let sensors: Vec<Box<dyn psyche::Sensor<Input = psyche::bus::Event> + Send + Sync>> = vec![
        Box::new(psyche::sensors::ChatSensor::default()),
        Box::new(psyche::sensors::ConnectionSensor::default()),
    ];

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma3".into());
    lingproc::ensure_model_available(&model).await?;
    info!("model {model} ready");
    let heart = psyche::Heart::new(vec![
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new(&model)),
            Some("fond".into()),
            std::time::Duration::from_secs(1),
        ),
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new(&model)),
            Some("wit2".into()),
            std::time::Duration::from_secs(2),
        ),
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new(&model)),
            Some("wit3".into()),
            std::time::Duration::from_secs(4),
        ),
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new(&model)),
            Some("quick".into()),
            std::time::Duration::from_secs(8),
        ),
    ]);

    let psyche = Arc::new(Mutex::new(psyche::Psyche::new(heart, sensors)));

    {
        let bus = bus.clone();
        let psyche = psyche.clone();
        tokio::spawn(async move {
            let mut rx = bus.subscribe();
            while let Ok(evt) = rx.recv().await {
                let mut p = psyche.lock().await;
                p.process_event(evt);
            }
        });
    }

    {
        let psyche = psyche.clone();
        tokio::spawn(async move {
            loop {
                {
                    let mut p = psyche.lock().await;
                    let _ = p.heart.tick();
                }
                tokio::task::yield_now().await;
            }
        });
    }

    info!("starting pete webserver");
    psyche::server::run_with_psyche(bus, psyche, ([127, 0, 0, 1], 8080)).await;
    Ok(())
}
