//! Launches the Pete web server exposing log and chat events over WebSockets.
use anyhow::Result;
use log::info;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Mutex;

use pete::web;

#[tokio::main]
async fn main() -> Result<()> {
    let bus = Arc::new(psyche::bus::EventBus::new());
    psyche::logging::init(bus.clone())?;

    let external_sensors: Vec<Box<dyn psyche::Sensor<Input = psyche::bus::Event> + Send + Sync>> = vec![
        Box::new(pete::sensors::ChatSensor::default()),
        Box::new(pete::sensors::ConnectionSensor::default()),
        // Box::new(pete::sensors::HeartbeatSensor::default()),
    ];

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma3".into());
    lingproc::ensure_model_available(&model).await?;
    info!("model {model} ready");
    let mut idx = 0;
    let bus_clone = bus.clone();
    let model_clone = model.clone();
    let make_sched = move || {
        idx += 1;
        let name = if idx == 1 { "quick" } else { "proc" };
        psyche::ProcessorScheduler::new(
            lingproc::OllamaProcessor::new(&model_clone),
            bus_clone.clone(),
            name,
        )
    };
    let psyche = Arc::new(Mutex::new(psyche::Psyche::new(
        make_sched,
        external_sensors,
    )));

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
        psyche::spawn_heartbeat(psyche);
    }

    {
        let bus = bus.clone();
        let psyche = psyche.clone();
        tokio::spawn(async move {
            if let Err(e) = web::serve(bus, psyche).await {
                log::error!("web server error: {e}");
            }
        });
    }

    info!("pete running on http://localhost:8080");
    signal::ctrl_c().await?;
    Ok(())
}
