use clap::Parser;
use core::{psyche::Psyche, witness::WitnessAgent};
use dotenvy::dotenv;
use llm::model_from_env;
use memory::{GraphRag, Memory};
use net::stream_bus::{StreamBus, StreamEvent};
use log::LevelFilter;
use std::{env, sync::Arc, time::Duration};
use tokio::sync::{mpsc, Mutex};
use voice::model::OllamaClient;
use voice::ChatVoice;
use llm::OllamaClient as LlmClient;
mod logger;
mod server;

#[derive(Parser)]
struct Args {
    /// Tick rate in seconds
    #[arg(long)]
    tick_rate: Option<f32>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let logger = logger::SimpleLogger::init(LevelFilter::Info);
    let args = Args::parse();
    let rate = runtime::tick_rate(args.tick_rate);

    let ollama_url = env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into());
    let llm = OllamaClient::new(&ollama_url);
    let model = model_from_env();
    let narrator = ChatVoice::new(llm, model, 10);

    let memory_llm = LlmClient::new(&ollama_url);
    let qdrant_url = env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".into());
    let neo_uri = env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".into());
    let neo_user = env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".into());
    let neo_pass = env::var("NEO4J_PASS").unwrap_or_else(|_| "neo4j".into());
    let memory = GraphRag::new(&qdrant_url, &neo_uri, &neo_user, &neo_pass).expect("connect memory");
    let bus = StreamBus::new(16);

    let witness = WitnessAgent::default();
    let mut psyche = Psyche::new(witness, narrator);
    psyche.agent.self_understanding = Some("I am Pete Daringsby.".into());
    let (tx, mut rx) = mpsc::channel(8);
    let mood = Arc::new(Mutex::new(String::from("\u{1F610}")));
    let server = server::router(tx.clone(), mood.clone(), logger.clone());
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 3000))
            .await
            .expect("bind server");
        axum::serve(listener, server).await.unwrap();
    });

    let delay = Duration::from_secs_f32(rate);
    loop {
        while let Ok(s) = rx.try_recv() {
            match psyche.witness.feel(s, &memory_llm).await {
                Ok(exp) => {
                    let _ = memory.store(exp.clone()).await;
                    let _ = bus.send(StreamEvent::MemoryUpdate { summary: exp.explanation.clone() });
                }
                Err(e) => log::error!("feel error: {:?}", e),
            }
        }
        let output = psyche.tick().await;
        {
            let mut m = mood.lock().await;
            *m = psyche.mood.clone();
        }
        if !output.think.content.is_empty() {
            log::info!("Think: {}", output.think.content);
        }
        if let Some(say) = output.say {
            log::info!("Pete: {}", say.content);
        }
        tokio::time::sleep(delay).await;
    }
}
