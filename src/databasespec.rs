use serde::{Deserialize, Serialize};

use crate::CommandSettings;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Database {
    pub(crate) command_settings: CommandSettings,
    // pub(crate) local_nodes: Vec<LocalNode>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LocalNodeType {
    #[default]
    Local,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct LocalNode {
    pub name: String,
    pub node_type: LocalNodeType,
    pub path: String,
}
