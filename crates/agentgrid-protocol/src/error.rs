use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub ok: bool,
    pub error: ProtocolError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}
