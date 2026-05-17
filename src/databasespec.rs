use serde::{Deserialize, Serialize};

use crate::CommandSettings;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Database {
    pub(crate) command_settings: CommandSettings,
}
