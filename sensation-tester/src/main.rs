use clap::Parser;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::SinkExt;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    geo: Option<Vec<f64>>,
    #[arg(long)]
    text: Option<String>,
    #[arg(default_value = "ws://localhost:8000/ws")]
    url: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let (mut ws, _) = connect_async(&args.url).await.expect("connect");
    if let Some(v) = args.geo {
        if v.len() == 2 {
            let msg = serde_json::json!({"sensor_type":"geolocation","lat":v[0],"lon":v[1]});
            ws.send(Message::Text(msg.to_string())).await.unwrap();
        }
    }
    if let Some(t) = args.text {
        let msg = serde_json::json!({"sensor_type":"text","value":t});
        ws.send(Message::Text(msg.to_string())).await.unwrap();
    }
}
