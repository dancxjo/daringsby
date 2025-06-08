use clap::Parser;
use dotenvy::dotenv;
use std::{env, time::Duration};
use core::{psyche::Psyche, witness::WitnessAgent};
use voice::ChatVoice;
use llm::model_from_env;
use voice::model::OllamaClient;
use tokio::sync::mpsc;
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
    env_logger::init();
    let args = Args::parse();
    let rate = runtime::tick_rate(args.tick_rate);

    let ollama_url = env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into());
    let llm = OllamaClient::new(&ollama_url);
    let model = model_from_env();
    let narrator = ChatVoice::new(llm, model, 10);

    let witness = WitnessAgent::default();
    let mut psyche = Psyche::new(witness, narrator);
    psyche.agent.self_understanding = Some("I am Pete Daringsby.".into());
    let (tx, mut rx) = mpsc::channel(8);
    let server = server::router(tx);
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 3000))
            .await
            .expect("bind server");
        axum::serve(listener, server).await.unwrap();
    });

    let delay = Duration::from_secs_f32(rate);
    loop {
        while let Ok(s) = rx.try_recv() {
            psyche.witness.ingest(s);
        }
        let output = psyche.tick().await;
        if let Some(say) = output.say {
            println!("Pete: {}", say.content);
        }
        tokio::time::sleep(delay).await;
    }
}
