use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageData {
    pub mime: String,
    pub base64: String,
}
