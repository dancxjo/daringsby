use quick_xml::{Reader, events::Event};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Discrete actions the host can execute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instruction {
    /// Speak `text` optionally using a named `voice`.
    Say { voice: Option<String>, text: String },
    /// Change the expressed emotion.
    Emote(String),
    /// Move to the named location.
    Move { to: String },
    /// End the current episode and summarize it.
    BreakEpisode,
}

/// Parse a list of [`Instruction`]s from a short XML snippet.
///
/// Unknown tags are ignored with a debug log.
///
/// # Examples
/// ```
/// use psyche::{parse_instructions, Instruction};
/// let out = "<say voice=\"kind\">Hi</say><emote>😊</emote>";
/// let items = parse_instructions(out);
/// assert_eq!(items[0], Instruction::Say { voice: Some("kind".into()), text: "Hi".into() });
/// assert_eq!(items[1], Instruction::Emote("😊".into()));
/// ```
pub fn parse_instructions(text: &str) -> Vec<Instruction> {
    let mut reader = Reader::from_str(text);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut current: Option<(String, HashMap<String, String>)> = None;
    let mut content = String::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = HashMap::new();
                for a in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    let val = a.unescape_value().unwrap_or_default().to_string();
                    attrs.insert(key, val);
                }
                current = Some((name, attrs));
            }
            Ok(Event::Text(t)) => {
                if current.is_some() {
                    content.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(_)) => {
                if let Some((name, attrs)) = current.take() {
                    match name.as_str() {
                        "say" => out.push(Instruction::Say {
                            voice: attrs.get("voice").cloned(),
                            text: content.clone(),
                        }),
                        "emote" => out.push(Instruction::Emote(content.clone())),
                        "move" => out.push(Instruction::Move {
                            to: attrs.get("to").cloned().unwrap_or_default(),
                        }),
                        "break-episode" => out.push(Instruction::BreakEpisode),
                        other => debug!(%other, "unknown instruction tag"),
                    }
                    content.clear();
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = HashMap::new();
                for a in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    let val = a.unescape_value().unwrap_or_default().to_string();
                    attrs.insert(key, val);
                }
                match name.as_str() {
                    "move" => out.push(Instruction::Move {
                        to: attrs.get("to").cloned().unwrap_or_default(),
                    }),
                    "break-episode" => out.push(Instruction::BreakEpisode),
                    other => debug!(%other, "unknown empty instruction"),
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                debug!(error = ?e, "error parsing instructions");
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    out
}
