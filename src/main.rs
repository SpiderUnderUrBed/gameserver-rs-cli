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
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::Request},
};
 use tokio::io::AsyncBufReadExt;

use crate::{
    databasespec::Database,
    gameserverspec::{Node, NodeArgs, RetrieveElement, Server, ServerArgs, SettingsArgs, UserArgs},
    jsondatabase::{ensure_db, load_db, save_db},
    types::{ChangeNodeRequest, IncomingMessage, IncomingMessageWithValue},
};
use gameserverspec::Settings;

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
        }
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

#[tokio::main]
async fn main() {
    let cli = Main::parse();
    let client = Client::new();
    let mut state = AppState::default();
    let _ = ensure_db();
    state.db = load_db();

    match cli.command {
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
                        eprintln!("warning: not running as root, journalctl may not be able to read the journal for '{}'", service);
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
