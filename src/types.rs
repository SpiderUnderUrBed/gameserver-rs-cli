use clap_derive::Args;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IncomingMessageWithValue {
    pub(crate) message: Value,
    #[serde(rename = "type")]
    pub(crate) message_type: String,
    pub(crate) authcode: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Args)]
pub struct ChangeNodeRequest {
    pub(crate) node_id: String,
    pub(crate) server_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IncomingMessage {
    pub(crate) message: String,
    #[serde(rename = "type")]
    pub(crate) message_type: String,
    pub(crate) authcode: String,
}
