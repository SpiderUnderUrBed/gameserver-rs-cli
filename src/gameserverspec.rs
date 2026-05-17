use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

#[derive(Debug)]
pub struct DatabaseError(pub u16);

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HTTP error: {}", self.0)
    }
}

impl Error for DatabaseError {}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum Filters {
    AlternatingLine,
    None,
}

impl FromStr for Filters {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "alternating_line" => Filters::AlternatingLine,
            _ => Filters::None,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum FileSystemDrivers {
    Tcp,
    None,
}

impl FromStr for FileSystemDrivers {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "tcp" => FileSystemDrivers::Tcp,
            _ => FileSystemDrivers::None,
        })
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeStatus {
    #[default]
    Unknown,
    Enabled,
    Disabled,
    ImmutablyEnabled,
    ImmutablyDisabled,
}

impl FromStr for NodeStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "enabled" => NodeStatus::Enabled,
            "disabled" => NodeStatus::Disabled,
            "immutably_enabled" | "immutablyenabled" => NodeStatus::ImmutablyEnabled,
            "immutably_disabled" | "immutablydisabled" => NodeStatus::ImmutablyDisabled,
            _ => NodeStatus::Unknown,
        })
    }
}

impl<'de> Deserialize<'de> for NodeStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        let s = match &v {
            Value::String(s) => s.clone(),
            Value::Object(map) => map
                .get("kind")
                .and_then(|k| k.as_str())
                .unwrap_or("unknown")
                .to_string(),
            _ => "unknown".to_string(),
        };
        Ok(NodeStatus::from_str(&s).unwrap_or_default())
    }
}

#[derive(Debug, Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum K8sType {
    Node,
    Pod,
    #[default]
    None,
    Inbuilt,
    Unknown,
}

impl FromStr for K8sType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "node" => K8sType::Node,
            "pod" => K8sType::Pod,
            "inbuilt" => K8sType::Inbuilt,
            "unknown" => K8sType::Unknown,
            _ => K8sType::None,
        })
    }
}

impl<'de> Deserialize<'de> for K8sType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(K8sType::from_str(&s).unwrap_or_default())
    }
}

#[derive(Debug, Default, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeType {
    #[default]
    Unknown,
    Custom,
    CustomWithString(String),
    InbuiltWithString(String),
    Inbuilt,
    Main,
}

impl FromStr for NodeType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(NodeType::from(s.to_lowercase()))
    }
}

impl<'de> Deserialize<'de> for NodeType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        match &v {
            Value::String(s) => Ok(NodeType::from(s.to_lowercase())),
            Value::Object(map) => {
                if let Some(Value::String(kind)) = map.get("kind") {
                    Ok(NodeType::from(kind.to_lowercase()))
                } else {
                    Ok(NodeType::Unknown)
                }
            }
            _ => Ok(NodeType::Unknown),
        }
    }
}

impl From<String> for NodeType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "custom" => NodeType::Custom,
            "inbuilt" => NodeType::Inbuilt,
            "main" => NodeType::Main,
            other => NodeType::CustomWithString(other.to_string()),
        }
    }
}

impl ToString for NodeType {
    fn to_string(&self) -> String {
        match self {
            NodeType::Custom => "custom".to_string(),
            NodeType::Inbuilt => "inbuilt".to_string(),
            NodeType::Main => "main".to_string(),
            NodeType::CustomWithString(s) => s.clone(),
            NodeType::InbuiltWithString(s) => s.clone(),
            _ => String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "lowercase", tag = "kind", content = "data")]
pub enum Intergrations {
    Minecraft,
    Other,
    #[default]
    Unknown,
}

impl FromStr for Intergrations {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "minecraft" => Intergrations::Minecraft,
            _ => Intergrations::Unknown,
        })
    }
}

impl ToString for Intergrations {
    fn to_string(&self) -> String {
        match self {
            Intergrations::Minecraft => "minecraft".to_string(),
            _ => "unknown".to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct UserPerm {
    pub perm: String,
    pub scope: String,
}

impl fmt::Display for UserPerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.perm, self.scope)
    }
}

impl FromStr for UserPerm {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (perm, scope) = s
            .split_once(':')
            .ok_or_else(|| format!("invalid perm format, expected 'perm:scope', got '{s}'"))?;
        Ok(UserPerm {
            perm: perm.to_string(),
            scope: scope.to_string(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
    pub user_perms: Vec<UserPerm>,
}

#[derive(Args, Debug)]
pub struct UserArgs {
    #[arg(long)]
    pub username: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "comma-separated list of perm:scope pairs"
    )]
    pub user_perms: Option<Vec<UserPerm>>,
}

impl UserArgs {
    pub fn into_user(self) -> User {
        User {
            username: self.username.unwrap_or_default(),
            password_hash: self.password,
            user_perms: self.user_perms.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodestatus: NodeStatus,
    pub nodetype: NodeType,
    pub k8s_type: K8sType,
}

#[derive(Args, Debug)]
pub struct NodeArgs {
    #[arg(long)]
    pub nodename: Option<String>,
    #[arg(long)]
    pub ip: Option<String>,
    #[arg(long)]
    pub nodestatus: Option<NodeStatus>,
    #[arg(long)]
    pub nodetype: Option<NodeType>,
    #[arg(long)]
    pub k8s_type: Option<K8sType>,
}

impl NodeArgs {
    pub fn into_node(self) -> Node {
        Node {
            nodename: self.nodename.unwrap_or_default(),
            ip: self.ip.unwrap_or_default(),
            nodestatus: self.nodestatus.unwrap_or_default(),
            nodetype: self.nodetype.unwrap_or_default(),
            k8s_type: self.k8s_type.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: String,
}

#[derive(Args, Debug)]
pub struct ButtonArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub link: Option<String>,
    #[arg(long, name = "type")]
    pub button_type: Option<String>,
}

impl ButtonArgs {
    pub fn into_button(self) -> Button {
        Button {
            name: self.name.unwrap_or_default(),
            link: self.link.unwrap_or_default(),
            r#type: self.button_type.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Intergration {
    pub status: String,
    pub r#type: Intergrations,
    pub settings: Value,
}

#[derive(Args, Debug)]
pub struct IntergrationArgs {
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long, name = "intergration-type")]
    pub intergration_type: Option<String>,
    #[arg(long, help = "JSON string of integration settings")]
    pub settings: Option<String>,
}

impl IntergrationArgs {
    pub fn into_intergration(self) -> Intergration {
        Intergration {
            status: self.status.unwrap_or_default(),
            r#type: self
                .intergration_type
                .and_then(|s| s.parse().ok())
                .unwrap_or_default(),
            settings: self
                .settings
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ServerMetadata {
    pub start_keyword: Option<String>,
    pub stop_keyword: Option<String>,
}

#[derive(Args, Debug)]
pub struct ServerMetadataArgs {
    #[arg(long)]
    pub start_keyword: Option<String>,
    #[arg(long)]
    pub stop_keyword: Option<String>,
}

impl ServerMetadataArgs {
    pub fn into_server_metadata(self) -> ServerMetadata {
        ServerMetadata {
            start_keyword: self.start_keyword,
            stop_keyword: self.stop_keyword,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct Server {
    #[serde(default)]
    pub servername: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub providertype: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub node: Node,
    #[serde(default)]
    pub sandbox: bool,
    #[serde(default)]
    pub server_metadata: ServerMetadata,
}

#[derive(Args, Debug)]
pub struct ServerArgs {
    #[arg(long)]
    pub servername: Option<String>,
    #[arg(long)]
    pub provider: Option<String>,
    #[arg(long)]
    pub providertype: Option<String>,
    #[arg(long)]
    pub location: Option<String>,
    #[arg(long, help = "JSON string of the node")]
    pub node: Option<String>,
    #[arg(long)]
    pub sandbox: Option<bool>,
    #[arg(long, help = "JSON string of server metadata")]
    pub server_metadata: Option<String>,
}

impl ServerArgs {
    pub fn into_server(self) -> Server {
        Server {
            servername: self.servername.unwrap_or_default(),
            provider: self.provider.unwrap_or_default(),
            providertype: self.providertype.unwrap_or_default(),
            location: self.location.unwrap_or_default(),
            node: self
                .node
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            sandbox: self.sandbox.unwrap_or_default(),
            server_metadata: self
                .server_metadata
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Settings {
    pub toggled_default_buttons: bool,
    pub status_type: String,
    pub enabled_rcon: bool,
    pub rcon_url: String,
    pub rcon_password: String,
    pub filter: Filters,
    pub file_system_driver: FileSystemDrivers,
    pub enable_statistics_on_home_page: bool,
    pub enable_nodes_on_home_page: bool,
    pub console_entry_on_top: bool,
    pub current_server: Server,
}

impl Default for Filters {
    fn default() -> Self {
        Filters::None
    }
}

impl Default for FileSystemDrivers {
    fn default() -> Self {
        FileSystemDrivers::None
    }
}

#[derive(Args, Debug)]
pub struct SettingsArgs {
    #[arg(long)]
    pub toggled_default_buttons: Option<bool>,
    #[arg(long)]
    pub status_type: Option<String>,
    #[arg(long)]
    pub enabled_rcon: Option<bool>,
    #[arg(long)]
    pub rcon_url: Option<String>,
    #[arg(long)]
    pub rcon_password: Option<String>,
    #[arg(long)]
    pub filter: Option<Filters>,
    #[arg(long)]
    pub file_system_driver: Option<FileSystemDrivers>,
    #[arg(long)]
    pub enable_statistics_on_home_page: Option<bool>,
    #[arg(long)]
    pub enable_nodes_on_home_page: Option<bool>,
    #[arg(long)]
    pub console_entry_on_top: Option<bool>,
    #[arg(long, help = "JSON string of the current server")]
    pub current_server: Option<String>,
}

impl SettingsArgs {
    pub fn into_settings(self) -> Settings {
        Settings {
            toggled_default_buttons: self.toggled_default_buttons.unwrap_or(false),
            status_type: self.status_type.unwrap_or_default(),
            enabled_rcon: self.enabled_rcon.unwrap_or(true),
            rcon_url: self
                .rcon_url
                .unwrap_or_else(|| "localhost:25575".to_string()),
            rcon_password: self.rcon_password.unwrap_or_else(|| "testing".to_string()),
            filter: self.filter.unwrap_or_default(),
            file_system_driver: self.file_system_driver.unwrap_or_default(),
            enable_statistics_on_home_page: self.enable_statistics_on_home_page.unwrap_or(false),
            enable_nodes_on_home_page: self.enable_nodes_on_home_page.unwrap_or(false),
            console_entry_on_top: self.console_entry_on_top.unwrap_or(true),
            current_server: self
                .current_server
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
        }
    }

    pub fn merge(self, other: Settings) -> Settings {
        Settings {
            toggled_default_buttons: self
                .toggled_default_buttons
                .unwrap_or(other.toggled_default_buttons),
            status_type: self.status_type.unwrap_or(other.status_type),
            enabled_rcon: self.enabled_rcon.unwrap_or(other.enabled_rcon),
            rcon_url: self.rcon_url.unwrap_or(other.rcon_url),
            rcon_password: self.rcon_password.unwrap_or(other.rcon_password),
            filter: self.filter.unwrap_or(other.filter),
            file_system_driver: self.file_system_driver.unwrap_or(other.file_system_driver),
            enable_statistics_on_home_page: self
                .enable_statistics_on_home_page
                .unwrap_or(other.enable_statistics_on_home_page),
            enable_nodes_on_home_page: self
                .enable_nodes_on_home_page
                .unwrap_or(other.enable_nodes_on_home_page),
            console_entry_on_top: self
                .console_entry_on_top
                .unwrap_or(other.console_entry_on_top),
            current_server: self
                .current_server
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(other.current_server),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RetrieveElement {
    pub element: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
pub enum Element {
    User {
        password: String,
        user: String,
        user_perms: Vec<UserPerm>,
    },
    Node(Node),
    Button(Button),
    Server(Server),
    Intergration(Intergration),
    String(String),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModifyElementData {
    pub element: Element,
    pub jwt: String,
    pub require_auth: bool,
}
