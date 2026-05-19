mod databasespec;
mod gameserverspec;
mod jsondatabase;
mod types;

use clap::Parser;
use clap_derive::{Args, Subcommand};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tokio::io::AsyncBufReadExt;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::Request},
};

use crate::{
    databasespec::{Database, LocalNode, LocalNodeType},
    gameserverspec::{Node, NodeArgs, RetrieveElement, Server, ServerArgs, SettingsArgs, UserArgs},
    jsondatabase::{ensure_db, load_db, save_db},
    types::{ChangeNodeRequest, IncomingMessage, IncomingMessageWithValue},
};
use gameserverspec::Settings;

#[derive(Debug, Clone, PartialEq)]
pub enum AutoSchedule {
    ByPath,
    Manual,
    ByAvailability,
}

static AUTO_SCHEDULE: AutoSchedule = AutoSchedule::ByAvailability;
static FORCE_LOCATION_IN_SERVER: bool = false;
static REQUIRE_SERVER_NAME: bool = false;
static USES_CWD_AS_SERVERNAME: bool = true;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Main {
    #[clap(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    State(StateCmd),
    Stream(StreamCmd),
    Settings(SettingsCmd),
    Users(UsersCmd),
    Nodes(NodesCmd),
    Server(ServerCmd),
    CommandSettings(CommandSettingsCmd),
    FsOperation(FsOperationCmd),
    // #[command(name = "gameserver", next_help_heading = "Presets")]
    Presets(GameserverCmd),
}

#[derive(Debug, Args)]
pub struct FsOperationCmd {
    #[clap(subcommand)]
    pub action: FsOperationType,
}

#[derive(Debug, Args)]
pub struct StreamCmd {
    #[clap(subcommand)]
    pub action: StreamType,
    #[arg(long, global = true)]
    pub no_json: bool,
    #[arg(long, global = true)]
    pub no_duplicates: bool,
    #[arg(long, global = true)]
    pub output_systemd: Option<String>,
}

#[derive(Debug, Args)]
pub struct StateCmd {
    #[clap(subcommand)]
    pub action: StateType,
}

#[derive(Debug, Args)]
pub struct CommandSettingsCmd {
    #[clap(subcommand)]
    pub action: CmdSettingsType,
}

#[derive(Debug, Args)]
pub struct NodesCmd {
    #[clap(subcommand)]
    pub action: NodesType,
}

#[derive(Debug, Args)]
pub struct ServerCmd {
    #[clap(subcommand)]
    pub action: ServersType,
}

#[derive(Debug, Args)]
pub struct SettingsCmd {
    #[clap(subcommand)]
    pub action: SettingsType,
}

#[derive(Debug, Args)]
pub struct UsersCmd {
    #[clap(subcommand)]
    pub action: UsersType,
}

#[derive(Debug, Args)]
pub struct GameserverCmd {
    #[clap(subcommand)]
    pub action: GameserverType,
}

#[derive(Debug, Subcommand)]
pub enum FsOperationType {}

#[derive(Debug, Subcommand)]
pub enum StateType {
    Get,
    Set(StateArgs),
}

#[derive(Debug, Subcommand)]
pub enum StreamType {
    Follow,
    Interact,
}

#[derive(Debug, Subcommand)]
pub enum CmdSettingsType {
    Set(CommandSettings),
    Get,
}

#[derive(Debug, Subcommand)]
pub enum GameserverType {
    Run(GameserverRunArgs),
}

#[derive(Debug, Args)]
pub struct GameserverRunArgs {
    #[arg(long)]
    pub file: Option<String>,
    #[arg(long)]
    pub node: Option<String>,
    #[arg(long)]
    pub start_cmd: String,
    #[arg(long)]
    pub servername: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ServersType {
    Create(ServerArgs),
    Get,
    Start,
    Stop,
    Delete(DeleteServerArgs),
    Set(SetServerArgs),
}

#[derive(Debug, Args)]
pub struct DeleteServerArgs {
    #[arg(long)]
    pub servername: String,
    #[arg(long, default_value_t = false)]
    pub delete_files: bool,
}

#[derive(Debug, Args)]
pub struct SetServerArgs {
    #[arg(long)]
    pub servername: String,
}

#[derive(Debug, Subcommand)]
pub enum NodesType {
    Create(NodeArgs),
    Get,
}

#[derive(Debug, Subcommand)]
pub enum SettingsType {
    Set(SettingsArgs),
    Get,
}

#[derive(Debug, Subcommand)]
pub enum UsersType {
    Create(UserArgs),
    Get,
}

#[derive(Debug, Args)]
pub struct StateArgs {
    #[arg(long)]
    pub node_id: Option<String>,
    #[arg(long)]
    pub server_id: Option<String>,
}

impl StateArgs {
    pub fn into_change_node_request(self) -> Option<ChangeNodeRequest> {
        match (self.node_id, self.server_id) {
            (Some(node_id), Some(server_id)) => Some(ChangeNodeRequest { node_id, server_id }),
            _ => None,
        }
    }
}

#[derive(Args, Debug, Default, Deserialize, Serialize)]
struct CommandSettings {
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long)]
    url: Option<String>,
    // Decribes the PID of the process not using the gameserver system
    #[arg(long, value_delimiter = ',', help = "comma-separated list of pids")]
    external_process_pid: Option<Vec<String>>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "comma-separated list of systemd service names"
    )]
    external_process_systemd: Option<Vec<String>>,
    #[arg(long)]
    forward_actions_url: Option<String>,
    #[arg(long)]
    forward_actions: Option<String>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "comma-separated list of name:path pairs e.g. mynode:/srv/gameserver"
    )]
    local_nodes: Option<Vec<String>>,
}

impl CommandSettings {
    pub fn merge(self, other: CommandSettings) -> CommandSettings {
        CommandSettings {
            auth_token: self.auth_token.or(other.auth_token),
            url: self.url.or(other.url),
            external_process_pid: self.external_process_pid.or(other.external_process_pid),
            external_process_systemd: self
                .external_process_systemd
                .or(other.external_process_systemd),
            forward_actions_url: self.forward_actions_url.or(other.forward_actions_url),
            forward_actions: self.forward_actions.or(other.forward_actions),
            local_nodes: self.local_nodes.or(other.local_nodes),
        }
    }

    pub fn parsed_local_nodes(&self) -> Vec<LocalNode> {
        self.local_nodes
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|entry| {
                let parts: Vec<&str> = entry.splitn(3, ':').collect();
                let (name, node_type, raw_path) = match parts.as_slice() {
                    [name, path] => (*name, LocalNodeType::Local, *path),
                    [name, node_type, path] => {
                        let node_type = match *node_type {
                            "local" => LocalNodeType::Local,
                            other => {
                                eprintln!("Unknown node type '{}', defaulting to local", other);
                                LocalNodeType::Local
                            }
                        };
                        (*name, node_type, *path)
                    }
                    _ => return None,
                };

                let expanded = if raw_path.starts_with("~/") {
                    let home = std::env::var("HOME").unwrap_or_default();
                    format!("{}/{}", home.trim_end_matches('/'), &raw_path[2..])
                } else {
                    raw_path.to_string()
                };

                let mut path = std::fs::canonicalize(&expanded)
                    .unwrap_or_else(|_| std::path::PathBuf::from(&expanded))
                    .to_string_lossy()
                    .to_string();

                if !path.ends_with("/server") && !path.ends_with("/server/") {
                    path = format!("{}/server", path.trim_end_matches('/'));
                }

                Some(LocalNode {
                    name: name.to_string(),
                    node_type,
                    path,
                })
            })
            .collect()
    }
}

#[derive(Default)]
struct AppState {
    db: Database,
}

fn base_url(state: &AppState) -> String {
    let url = state.db.command_settings.url.clone().unwrap_or_default();
    if url.starts_with("http://") || url.starts_with("https://") {
        url
    } else {
        format!("http://{}", url)
    }
}

fn auth_header(state: &AppState) -> String {
    format!(
        "Bearer {}",
        state
            .db
            .command_settings
            .auth_token
            .clone()
            .unwrap_or_default()
    )
}

fn find_node_for_path<'a>(nodes: &'a [LocalNode], file: &str) -> Option<&'a LocalNode> {
    nodes.iter().find(|n| file.starts_with(&n.path))
}

#[tokio::main]
async fn main() {
    let cli = Main::parse();
    let client = Client::new();
    let mut state = AppState::default();
    let _ = ensure_db();
    state.db = load_db();

    match cli.command {
        Some(Commands::Presets(cmd)) => match cmd.action {
            GameserverType::Run(args) => {
                if REQUIRE_SERVER_NAME && args.servername.is_none() && !USES_CWD_AS_SERVERNAME {
                    eprintln!("--servername is required when REQUIRE_SERVER_NAME is true");
                    return;
                }

                let nodes = state.db.command_settings.parsed_local_nodes();

                let file = args.file.clone().unwrap_or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                });

                let servername = if USES_CWD_AS_SERVERNAME && args.servername.is_none() {
                    std::env::current_dir()
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .unwrap_or_else(|| "server".to_string())
                } else {
                    args.servername.clone().unwrap_or_else(|| {
                        let file_path = std::fs::canonicalize(&file)
                            .unwrap_or_else(|_| std::path::PathBuf::from(&file));
                        file_path
                            .parent()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "server".to_string())
                    })
                };

                let target_node = match &AUTO_SCHEDULE {
                    AutoSchedule::ByPath => find_node_for_path(&nodes, &file).cloned(),
                    AutoSchedule::Manual => match &args.node {
                        Some(name) => nodes.iter().find(|n| &n.name == name).cloned(),
                        None => {
                            eprintln!("--node is required when AUTO_SCHEDULE is Manual");
                            return;
                        }
                    },
                    AutoSchedule::ByAvailability => {
                        let get_settings_resp = client
                            .get(format!("{}/api/getsettings", base_url(&state)))
                            .header("Authorization", auth_header(&state))
                            .send()
                            .await;

                        if let Ok(r) = get_settings_resp {
                            if r.status().is_success() {
                                if let Ok(body) = r.json::<Settings>().await {
                                    let mut updated = serde_json::to_value(&body).unwrap_or_default();
                                    if let Some(obj) = updated.as_object_mut() {
                                        obj.insert(
                                            "status_type".to_string(),
                                            serde_json::json!("server-process"),
                                        );
                                    }
                                    let _ = client
                                        .post(format!("{}/api/setsettings", base_url(&state)))
                                        .header("Authorization", auth_header(&state))
                                        .json(&IncomingMessageWithValue {
                                            message: updated,
                                            message_type: String::new(),
                                            authcode: String::new(),
                                        })
                                        .send()
                                        .await;
                                }
                            }
                        }

                        let sse_url = format!("{}/api/awaitserverstatus", base_url(&state));
                        let mut chosen: Option<LocalNode> = None;

                        for node in &nodes {
                            let switch_resp = client
                                .put(format!("{}/api/changenode", base_url(&state)))
                                .header("Authorization", auth_header(&state))
                                .json(&ChangeNodeRequest {
                                    node_id: node.name.clone(),
                                    server_id: "none".to_string(),
                                })
                                .send()
                                .await;

                            if let Err(e) = switch_resp {
                                eprintln!("Failed to switch to node '{}': {}", node.name, e);
                                continue;
                            }

                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                            let sse_resp = client
                                .get(&sse_url)
                                .header("Authorization", auth_header(&state))
                                .send()
                                .await;

                            let status = match sse_resp {
                                Ok(mut r) if r.status().is_success() => {
                                    let mut status_str = String::new();
                                    let mut event_count = 0;
                                    'sse: loop {
                                        match tokio::time::timeout(
                                            tokio::time::Duration::from_secs(10),
                                            r.chunk(),
                                        )
                                        .await
                                        {
                                            Ok(Ok(Some(chunk))) => {
                                                let text = String::from_utf8_lossy(&chunk);
                                                for line in text.lines() {
                                                    if let Some(data) = line.strip_prefix("data:") {
                                                        event_count += 1;
                                                        status_str = data.trim().to_string();
                                                        // wait for at least 2 events so the first stale one is discarded
                                                        if event_count >= 2 {
                                                            break 'sse;
                                                        }
                                                    }
                                                }
                                            }
                                            _ => break 'sse,
                                        }
                                    }
                                    status_str
                                }
                                _ => {
                                    eprintln!("Failed to read SSE for node '{}'", node.name);
                                    continue;
                                }
                            };

                            if status == "down" || status == "unknown" {
                                println!("Scheduling to node '{}' (status: {})", node.name, status);
                                chosen = Some(node.clone());
                                break;
                            }

                            println!("Node '{}' reports status '{}', trying next", node.name, status);
                        }

                        match chosen {
                            Some(n) => Some(n),
                            None => {
                                eprintln!(
                                    "No available node found (all nodes are up or unreachable)"
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                };

                let node = match target_node {
                    Some(n) => n,
                    None => {
                        eprintln!("No local node found for file: {}", file);
                        return;
                    }
                };

                if !matches!(node.node_type, LocalNodeType::Local) {
                    eprintln!(
                        "Node '{}' is not a local node. Only local nodes are supported for presets run.",
                        node.name
                    );
                    return;
                }

                if FORCE_LOCATION_IN_SERVER {
                    let in_any_node = nodes.iter().any(|n| file.starts_with(&n.path));
                    if !in_any_node {
                        eprintln!(
                            "FORCE_LOCATION_IN_SERVER is set: file must be inside a local node server directory. Known paths: {}",
                            nodes
                                .iter()
                                .map(|n| n.path.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        return;
                    }
                }

                let server_root = std::fs::canonicalize(std::path::Path::new(&node.path))
                    .unwrap_or_else(|_| std::path::PathBuf::from(&node.path));

                let file_path = std::fs::canonicalize(&file)
                    .unwrap_or_else(|_| std::path::PathBuf::from(&file));

                let server_location = servername.clone();

                let file_name = file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let server_dir = server_root.join(&server_location);

                if let Err(e) = std::fs::create_dir_all(&server_dir) {
                    eprintln!("Failed to create server directory: {}", e);
                    return;
                }

                let provider_config = serde_json::json!({
                    "start": args.start_cmd,
                    "location": if args.file.is_some() { file_name.clone() } else { String::new() },
                    "needed_paths": [],
                    "needed_commands": []
                });

                let provider_json_path = server_dir.join("provider.json");
                if let Err(e) = std::fs::write(
                    &provider_json_path,
                    serde_json::to_string_pretty(&provider_config).unwrap(),
                ) {
                    eprintln!("Failed to write provider.json: {}", e);
                    return;
                }
                println!("Wrote provider.json to {}", provider_json_path.display());

                let file_dest = server_dir.join(&file_name);
                if args.file.is_some() {
                    if file_path != file_dest {
                        if let Err(e) = std::fs::copy(&file, &file_dest) {
                            eprintln!("Failed to copy file to server directory: {}", e);
                            return;
                        }
                        println!("Copied {} to {}", file, file_dest.display());
                    } else {
                        println!("File already in server directory, skipping copy");
                    }
                }

                let servers_response = client
                    .get(format!("{}/api/servers", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await;

                let server_exists = match servers_response {
                    Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
                        Ok(body) => body
                            .as_array()
                            .map(|arr| {
                                arr.iter().any(|s| {
                                    s.get("servername")
                                        .and_then(|n| n.as_str())
                                        .map(|n| n == servername)
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false),
                        Err(_) => false,
                    },
                    _ => false,
                };

                if server_exists {
                    println!("Server '{}' already exists, reusing it", servername);
                } else {
                    let create_response = client
                        .post(format!("{}/api/addserver", base_url(&state)))
                        .header("Authorization", auth_header(&state))
                        .json(&serde_json::json!({
                            "element": {
                                "kind": "Server",
                                "data": {
                                    "servername": servername,
                                    "provider": "custom",
                                    "providertype": "",
                                    "location": server_location,
                                    "sandbox": false,
                                    "server_metadata": {}
                                }
                            },
                            "jwt": "",
                            "require_auth": false
                        }))
                        .send()
                        .await;

                    match create_response {
                        Ok(r) if r.status().is_success() => {}
                        Ok(r) if r.status() == 409 => {
                            println!("Server '{}' already exists (409), reusing it", servername);
                        }
                        Ok(r) => {
                            eprintln!("addserver failed: {}", r.status());
                            return;
                        }
                        Err(e) => {
                            eprintln!("addserver request failed: {}", e);
                            return;
                        }
                    }
                }

                let set_response = client
                    .post(format!("{}/api/setserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .json(&serde_json::json!({
                        "element": {
                            "kind": "String",
                            "data": servername
                        },
                        "jwt": "",
                        "require_auth": false
                    }))
                    .send()
                    .await;

                match set_response {
                    Ok(r) if r.status().is_success() => {}
                    Ok(r) => {
                        eprintln!("setserver failed: {}", r.status());
                        return;
                    }
                    Err(e) => {
                        eprintln!("setserver request failed: {}", e);
                        return;
                    }
                }

                let start_response = client
                    .post(format!("{}/api/startserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await;

                match start_response {
                    Ok(r) if r.status().is_success() => println!("Server started"),
                    Ok(r) => eprintln!("startserver failed: {}", r.status()),
                    Err(e) => eprintln!("startserver request failed: {}", e),
                }
            }
        },
        Some(Commands::Settings(cmd)) => match cmd.action {
            SettingsType::Set(args) => {
                let response = client
                    .get(format!("{}/api/getsettings", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    let body = response.json::<Settings>().await.unwrap();
                    let settings = args.merge(body);

                    let response = client
                        .post(format!("{}/api/setsettings", base_url(&state)))
                        .header("Authorization", auth_header(&state))
                        .json(&IncomingMessageWithValue {
                            message: serde_json::to_value(settings).unwrap(),
                            message_type: String::new(),
                            authcode: String::new(),
                        })
                        .send()
                        .await
                        .unwrap();

                    if response.status().is_success() {
                        println!("Successfuly changed settings");
                    } else {
                        eprintln!("request failed: {}", response.status());
                    }
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            SettingsType::Get => {
                let response = client
                    .get(format!("{}/api/getsettings", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(body) => {
                            println!("{}", serde_yaml::to_string(&body).unwrap_or_default())
                        }
                        Err(err) => eprintln!("{:#?}", err),
                    }
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
        },
        Some(Commands::Users(cmd)) => match cmd.action {
            UsersType::Get => {
                let response = client
                    .get(format!("{}/api/users", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(body) => {
                            println!("{}", serde_yaml::to_string(&body).unwrap_or_default())
                        }
                        Err(err) => eprintln!("{:#?}", err),
                    }
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            UsersType::Create(user_args) => {
                let user = user_args.into_user();
                let response = client
                    .post(format!("{}/api/createuser", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .json(&serde_json::json!({
                        "element": {
                            "kind": "User",
                            "data": {
                                "user": user.username,
                                "password": user.password_hash.unwrap_or_default(),
                                "user_perms": user.user_perms
                            }
                        },
                        "jwt": "",
                        "require_auth": false
                    }))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    println!("User created successfully");
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
        },
        Some(Commands::Nodes(cmd)) => match cmd.action {
            NodesType::Create(node_args) => {
                let node = node_args.into_node();
                let response = client
                    .post(format!("{}/api/addnode", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .json(&serde_json::json!({
                        "element": {
                            "kind": "Node",
                            "data": node
                        },
                        "jwt": "",
                        "require_auth": false
                    }))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    println!("Node created successfully");
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            NodesType::Get => {
                let response = client
                    .get(format!("{}/api/nodes", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(body) => {
                            println!("{}", serde_yaml::to_string(&body).unwrap_or_default())
                        }
                        Err(err) => eprintln!("{:#?}", err),
                    }
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
        },
        Some(Commands::Server(cmd)) => match cmd.action {
            ServersType::Create(server_args) => {
                let server = server_args.into_server();
                let response = client
                    .post(format!("{}/api/addserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .json(&serde_json::json!({
                        "element": {
                            "kind": "Server",
                            "data": server
                        },
                        "jwt": "",
                        "require_auth": false
                    }))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    println!("Server created successfully");
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            ServersType::Get => {
                let response = client
                    .get(format!("{}/api/servers", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(body) => {
                            println!("{}", serde_yaml::to_string(&body).unwrap_or_default())
                        }
                        Err(err) => eprintln!("{:#?}", err),
                    }
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            ServersType::Start => {
                // /api/startserver is a POST with no body, auth via header
                let response = client
                    .post(format!("{}/api/startserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    println!("Server start command sent");
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            ServersType::Stop => {
                // /api/stopserver is a POST with no body, auth via header
                let response = client
                    .post(format!("{}/api/stopserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    println!("Server stop command sent");
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            ServersType::Delete(args) => {
                // /api/deleteserver expects IncomingMessageWithMetadata with MetadataTypes::DeleteServer
                let response = client
                    .post(format!("{}/api/deleteserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .json(&serde_json::json!({
                        "message": "",
                        "type": "command",
                        "authcode": "",
                        "metadata": {
                            "kind": "DeleteServer",
                            "data": {
                                "delete_server_name": args.servername,
                                "delete_server_files": args.delete_files
                            }
                        }
                    }))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    println!("Server deleted successfully");
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
            ServersType::Set(args) => {
                // /api/setserver expects ModifyElementData with Element::String(servername)
                let response = client
                    .post(format!("{}/api/setserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .json(&serde_json::json!({
                        "element": {
                            "kind": "String",
                            "data": args.servername
                        },
                        "jwt": "",
                        "require_auth": false
                    }))
                    .send()
                    .await
                    .unwrap();

                if response.status().is_success() {
                    println!("Server set to {}", args.servername);
                } else {
                    eprintln!("request failed: {}", response.status());
                }
            }
        },
        Some(Commands::State(cmd)) => match cmd.action {
            // Runs a bunch of fetches to several routes to kinda show the 'state' of things
            StateType::Get => {
                let node_response = client
                    .get(format!("{}/api/getcurrentnode", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .send()
                    .await
                    .unwrap();

                match node_response.json::<Node>().await {
                    Ok(body) => println!("{}", serde_yaml::to_string(&body).unwrap_or_default()),
                    Err(err) => eprintln!("{:#?}", err),
                }

                let current_server_response = client
                    .post(format!("{}/api/getserver", base_url(&state)))
                    .header("Authorization", auth_header(&state))
                    .json(&RetrieveElement {
                        element: String::new(),
                    })
                    .send()
                    .await
                    .unwrap();

                match current_server_response.json::<Server>().await {
                    Ok(body) => println!("{}", serde_yaml::to_string(&body).unwrap_or_default()),
                    Err(err) => eprintln!("{:#?}", err),
                }
            }
            StateType::Set(state_args) => {
                if let Some(request) = state_args.into_change_node_request() {
                    let response = client
                        .put(format!("{}/api/changenode", base_url(&state)))
                        .header("Authorization", auth_header(&state))
                        .json(&request)
                        .send()
                        .await
                        .unwrap();
                    if response.status().is_success() {
                        println!("Success");
                    } else {
                        eprintln!("{:#?}", response);
                    }
                }
            }
        },
        Some(Commands::CommandSettings(cmd)) => match cmd.action {
            CmdSettingsType::Set(command_settings) => {
                state.db.command_settings = command_settings.merge(state.db.command_settings);
                save_db(&state.db);
            }
            CmdSettingsType::Get => {
                println!(
                    "{}",
                    serde_yaml::to_string(&state.db.command_settings).unwrap_or_default()
                );
            }
        },
        Some(Commands::Stream(cmd)) => match cmd.action {
            StreamType::Follow => {
                let url = state.db.command_settings.url.unwrap();
                let ws_url = format!(
                    "{}/api/ws",
                    url.replace("http://", "ws://")
                        .replace("https://", "wss://")
                );

                let mut request = ws_url.into_client_request().unwrap();
                request.headers_mut().insert(
                    "Authorization",
                    format!(
                        "Bearer {}",
                        state.db.command_settings.auth_token.unwrap_or_default()
                    )
                    .parse()
                    .unwrap(),
                );

                let (ws_stream, _) = connect_async(request).await.expect("failed to connect");
                let (_, mut read) = ws_stream.split();

                let mut last_message: Option<String> = None;
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            let parsed = parse_text(text, cmd.no_json);
                            if parsed.is_empty() {
                                continue;
                            }
                            if cmd.no_duplicates {
                                if last_message.as_deref() == Some(&parsed) {
                                    continue;
                                }
                                last_message = Some(parsed.clone());
                            }
                            println!("{}", parsed);
                        }
                        Ok(Message::Binary(bin)) => {
                            println!("received binary: {:?}", bin);
                        }
                        Ok(Message::Close(_)) => {
                            println!("connection closed");
                            break;
                        }
                        Err(e) => {
                            eprintln!("error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            StreamType::Interact => {
                let url = state.db.command_settings.url.unwrap_or(String::new());
                let ws_url = format!(
                    "{}/api/ws",
                    url.replace("http://", "ws://")
                        .replace("https://", "wss://")
                );

                let mut request = ws_url.into_client_request().unwrap();
                request.headers_mut().insert(
                    "Authorization",
                    format!(
                        "Bearer {}",
                        state.db.command_settings.auth_token.unwrap_or_default()
                    )
                    .parse()
                    .unwrap(),
                );

                let (ws_stream, _) = connect_async(request).await.expect("failed to connect");
                let (mut write, mut read) = ws_stream.split();

                let no_json = cmd.no_json;
                let no_duplicates = cmd.no_duplicates;
                let output_systemd = cmd.output_systemd.clone();

                if let Some(ref service) = output_systemd {
                    if unsafe { libc::getuid() } != 0 {
                        eprintln!(
                            "warning: not running as root, journalctl may not be able to read the journal for '{}'",
                            service
                        );
                    }
                    let service = service.clone();
                    tokio::spawn(async move {
                        let mut child = tokio::process::Command::new("journalctl")
                            .args(["-u", &service, "-f", "-n", "0", "--output=cat"])
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                            .expect("failed to spawn journalctl");

                        if let Some(stdout) = child.stdout.take() {
                            let mut reader = tokio::io::BufReader::new(stdout).lines();
                            let mut last_message: Option<String> = None;
                            while let Ok(Some(line)) = reader.next_line().await {
                                let parsed = parse_text(line, no_json);
                                if parsed.is_empty() {
                                    continue;
                                }
                                if no_duplicates {
                                    if last_message.as_deref() == Some(&parsed) {
                                        continue;
                                    }
                                    last_message = Some(parsed.clone());
                                }
                                println!("{}", parsed);
                            }
                        }
                    });
                }

                tokio::spawn(async move {
                    let mut last_message: Option<String> = None;
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if output_systemd.is_some() {
                                    continue;
                                }
                                let parsed = parse_text(text, no_json);
                                if parsed.is_empty() {
                                    continue;
                                }
                                if no_duplicates {
                                    if last_message.as_deref() == Some(&parsed) {
                                        continue;
                                    }
                                    last_message = Some(parsed.clone());
                                }
                                println!("\r[server]: {}\n> ", parsed);
                            }
                            Ok(Message::Close(_)) => {
                                println!("connection closed");
                                break;
                            }
                            Err(e) => {
                                eprintln!("error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                });

                let stdin = tokio::io::stdin();
                let reader = tokio::io::BufReader::new(stdin);
                let mut lines = tokio::io::AsyncBufReadExt::lines(reader);

                print!("> ");
                std::io::Write::flush(&mut std::io::stdout()).unwrap();

                while let Ok(Some(line)) = lines.next_line().await {
                    let modified = modify_message(line);

                    if modified.is_empty() {
                        continue;
                    }

                    if modified == "/quit" {
                        break;
                    }

                    write.send(Message::Text(modified)).await.unwrap();

                    print!("> ");
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                }
            }
        },
        Some(Commands::FsOperation(_)) => {}
        None => {}
    }
}

fn parse_text(text: String, no_json: bool) -> String {
    if let Ok(full_message) = serde_json::from_str::<IncomingMessage>(&text) {
        return full_message.message;
    }
    let json_part = if let Some(idx) = text.find("Received JSON here line: ") {
        &text[idx + "Received JSON here line: ".len()..]
    } else {
        &text
    };
    if let Ok(outer) = serde_json::from_str::<serde_json::Value>(json_part) {
        if let Some(data_str) = outer.get("data").and_then(|d| d.as_str()) {
            if let Ok(inner) = serde_json::from_str::<serde_json::Value>(data_str) {
                if let Some(msg) = inner.get("data").and_then(|d| d.as_str()) {
                    return msg.to_string();
                }
            }
            return data_str.to_string();
        }
    }
    if no_json && text.trim_start().starts_with('{') {
        return String::new();
    }
    if no_json {
        return String::new();
    }
    text
}

// Dont think i need to modify the message
fn modify_message(msg: String) -> String {
    msg
}

// Looks for a env varible, if its not found, try the specified default, if none is found it will use the default of whatever that type is
fn get_env_var_or_arg<T>(env_var: &str, default: Option<T>) -> Option<T>
where
    T: std::str::FromStr + Clone,
{
    env::var(env_var)
        .ok()
        .and_then(|s| s.parse::<T>().ok())
        .or(default)
}