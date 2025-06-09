//! Launches the Pete web server exposing log and chat events over WebSockets.
use anyhow::Result;
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;

struct Echo;

impl psyche::Sensor for Echo {
    type Input = String;
    fn feel(&mut self, s: psyche::Sensation<Self::Input>) -> Option<psyche::Experience> {
        Some(psyche::Experience::new(s.what))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let bus = Arc::new(psyche::bus::EventBus::new());
    psyche::logging::init(bus.clone())?;

    let sensors: Vec<Box<dyn psyche::Sensor<Input = psyche::bus::Event> + Send + Sync>> = vec![
        Box::new(psyche::sensors::ChatSensor::default()),
        Box::new(psyche::sensors::ConnectionSensor::default()),
    ];

    let heart = psyche::Heart::new(vec![
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new("llama2")),
            Echo,
            Some("fond".into()),
            std::time::Duration::from_secs(1),
        ),
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new("llama2")),
            Echo,
            Some("wit2".into()),
            std::time::Duration::from_secs(2),
        ),
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new("llama2")),
            Echo,
            Some("wit3".into()),
            std::time::Duration::from_secs(4),
        ),
        psyche::Wit::with_config(
            psyche::ProcessorScheduler::new(lingproc::OllamaProcessor::new("llama2")),
            Echo,
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
            use tokio::time::{sleep, Duration};
            loop {
                {
                    let mut p = psyche.lock().await;
                    p.heart.tick();
                }
                sleep(Duration::from_secs(1)).await;
            }
        });
    }

    info!("starting pete webserver");
    psyche::server::run_with_psyche(bus, psyche, ([127, 0, 0, 1], 8080)).await;
    Ok(())
}
