//! Cliente JSON-RPC 2.0 do aria2 (POST em http://127.0.0.1:<porta>/jsonrpc).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("o motor de downloads não respondeu: {0}")]
    Transport(String),
    #[error("aria2 recusou o pedido ({code}): {message}")]
    Remote { code: i64, message: String },
    #[error("resposta inesperada do aria2: {0}")]
    Decode(String),
}

impl RpcError {
    /// O aria2 responde código 1 e "... is not found" quando o GID não existe.
    pub fn is_not_found(&self) -> bool {
        matches!(self, RpcError::Remote { message, .. } if message.contains("not found"))
    }
}

#[derive(Clone)]
pub struct Aria2Client {
    http: reqwest::Client,
    endpoint: String,
    token: String,
    next_id: std::sync::Arc<AtomicU64>,
}

impl std::fmt::Debug for Aria2Client {
    // O segredo do RPC nunca aparece em logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Aria2Client").field("endpoint", &self.endpoint).finish_non_exhaustive()
    }
}

/// Campos que o SHIFT usa do `aria2.tellStatus`.
pub const STATUS_KEYS: &[&str] = &[
    "gid", "status", "totalLength", "completedLength", "downloadSpeed", "connections", "errorCode",
    "errorMessage", "files", "followedBy", "bittorrent", "numSeeders", "uploadSpeed", "uploadLength", "seeder",
    "infoHash",
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Aria2File {
    pub path: String,
    pub length: u64,
    pub completed: u64,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Aria2Status {
    pub gid: String,
    /// active | waiting | paused | error | complete | removed
    pub status: String,
    pub total_length: u64,
    pub completed_length: u64,
    pub download_speed: u64,
    pub connections: u32,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub path: Option<String>,
    pub files: Vec<Aria2File>,
    /// Magnet: o download de metadados é seguido pelo download do torrent (novo GID).
    pub followed_by: Vec<String>,
    pub bt_name: Option<String>,
    pub info_hash: Option<String>,
    pub num_seeders: u32,
    pub upload_speed: u64,
    pub upload_length: u64,
    /// Terminou de baixar e está semeando.
    pub seeder: bool,
}

#[derive(Deserialize)]
struct RawStatus {
    gid: String,
    status: String,
    #[serde(rename = "totalLength", default)]
    total_length: Option<String>,
    #[serde(rename = "completedLength", default)]
    completed_length: Option<String>,
    #[serde(rename = "downloadSpeed", default)]
    download_speed: Option<String>,
    #[serde(default)]
    connections: Option<String>,
    #[serde(rename = "errorCode", default)]
    error_code: Option<String>,
    #[serde(rename = "errorMessage", default)]
    error_message: Option<String>,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(rename = "followedBy", default)]
    followed_by: Vec<String>,
    #[serde(default)]
    bittorrent: Option<RawBt>,
    #[serde(rename = "infoHash", default)]
    info_hash: Option<String>,
    #[serde(rename = "numSeeders", default)]
    num_seeders: Option<String>,
    #[serde(rename = "uploadSpeed", default)]
    upload_speed: Option<String>,
    #[serde(rename = "uploadLength", default)]
    upload_length: Option<String>,
    #[serde(default)]
    seeder: Option<String>,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    path: String,
    #[serde(default)]
    length: Option<String>,
    #[serde(rename = "completedLength", default)]
    completed_length: Option<String>,
    #[serde(default)]
    selected: Option<String>,
}

#[derive(Deserialize)]
struct RawBt {
    #[serde(default)]
    info: Option<RawBtInfo>,
}

#[derive(Deserialize)]
struct RawBtInfo {
    #[serde(default)]
    name: Option<String>,
}

fn num<T: std::str::FromStr + Default>(s: &Option<String>) -> T {
    s.as_deref().and_then(|v| v.parse().ok()).unwrap_or_default()
}

impl From<RawStatus> for Aria2Status {
    fn from(r: RawStatus) -> Self {
        Self {
            total_length: num(&r.total_length),
            completed_length: num(&r.completed_length),
            download_speed: num(&r.download_speed),
            connections: num(&r.connections),
            error_code: r.error_code.filter(|c| c != "0"),
            error_message: r.error_message.filter(|m| !m.is_empty()),
            path: r.files.first().map(|f| f.path.clone()).filter(|p| !p.is_empty()),
            files: r
                .files
                .iter()
                .map(|f| Aria2File {
                    path: f.path.clone(),
                    length: num(&f.length),
                    completed: num(&f.completed_length),
                    selected: f.selected.as_deref() != Some("false"),
                })
                .collect(),
            followed_by: r.followed_by,
            bt_name: r.bittorrent.and_then(|b| b.info).and_then(|i| i.name),
            info_hash: r.info_hash,
            num_seeders: num(&r.num_seeders),
            upload_speed: num(&r.upload_speed),
            upload_length: num(&r.upload_length),
            seeder: r.seeder.as_deref() == Some("true"),
            gid: r.gid,
            status: r.status,
        }
    }
}

impl Aria2Client {
    pub fn new(port: u16, token: &str) -> Self {
        let http = reqwest::Client::builder()
            .no_proxy() // RPC local nunca passa por proxy do sistema
            .timeout(Duration::from_secs(10))
            .build()
            .expect("cliente HTTP local");
        Self {
            http,
            endpoint: format!("http://127.0.0.1:{port}/jsonrpc"),
            token: format!("token:{token}"),
            next_id: Default::default(),
        }
    }

    async fn post(&self, body: Value) -> Result<Value, RpcError> {
        let res = self.http.post(&self.endpoint).json(&body).send().await.map_err(|e| RpcError::Transport(e.to_string()))?;
        let v: Value = res.json().await.map_err(|e| RpcError::Decode(e.to_string()))?;
        if let Some(err) = v.get("error") {
            return Err(RpcError::Remote {
                code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
                message: err.get("message").and_then(Value::as_str).unwrap_or("").to_string(),
            });
        }
        v.get("result").cloned().ok_or_else(|| RpcError::Decode("sem campo result".into()))
    }

    pub async fn call(&self, method: &str, params: Vec<Value>) -> Result<Value, RpcError> {
        let mut all = Vec::with_capacity(params.len() + 1);
        all.push(Value::String(self.token.clone()));
        all.extend(params);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.post(json!({ "jsonrpc": "2.0", "id": id.to_string(), "method": method, "params": all })).await
    }

    pub async fn version(&self) -> Result<String, RpcError> {
        let v = self.call("aria2.getVersion", vec![]).await?;
        Ok(v.get("version").and_then(Value::as_str).unwrap_or("?").to_string())
    }

    /// Adiciona um download com GID escolhido pelo chamador. `options` usa os nomes
    /// de opção do aria2 (dir, out, split, max-connection-per-server, pause…).
    pub async fn add_uri(&self, uris: &[String], options: Map<String, Value>) -> Result<String, RpcError> {
        let v = self.call("aria2.addUri", vec![json!(uris), Value::Object(options)]).await?;
        v.as_str().map(str::to_string).ok_or_else(|| RpcError::Decode("GID ausente".into()))
    }

    /// Adiciona um .torrent (conteúdo em base64, como a API pede).
    pub async fn add_torrent(&self, torrent_base64: &str, options: Map<String, Value>) -> Result<String, RpcError> {
        let v = self.call("aria2.addTorrent", vec![json!(torrent_base64), json!([]), Value::Object(options)]).await?;
        v.as_str().map(str::to_string).ok_or_else(|| RpcError::Decode("GID ausente".into()))
    }

    pub async fn tell_status(&self, gid: &str) -> Result<Aria2Status, RpcError> {
        let v = self.call("aria2.tellStatus", vec![json!(gid), json!(STATUS_KEYS)]).await?;
        let raw: RawStatus = serde_json::from_value(v).map_err(|e| RpcError::Decode(e.to_string()))?;
        Ok(raw.into())
    }

    /// Vários `tellStatus` numa só ida ao aria2. GIDs desconhecidos voltam como `None`.
    pub async fn tell_many(&self, gids: &[String]) -> Result<Vec<Option<Aria2Status>>, RpcError> {
        if gids.is_empty() {
            return Ok(vec![]);
        }
        let calls: Vec<Value> = gids
            .iter()
            .map(|g| json!({ "methodName": "aria2.tellStatus", "params": [self.token, g, STATUS_KEYS] }))
            .collect();
        // system.multicall não recebe o token no topo: cada chamada leva o seu.
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let v = self.post(json!({ "jsonrpc": "2.0", "id": id.to_string(), "method": "system.multicall", "params": [calls] })).await?;
        let items = v.as_array().ok_or_else(|| RpcError::Decode("multicall".into()))?;
        Ok(items
            .iter()
            .map(|item| {
                // Sucesso vem como [resultado]; falha como {code, message}.
                item.as_array()
                    .and_then(|a| a.first())
                    .and_then(|r| serde_json::from_value::<RawStatus>(r.clone()).ok())
                    .map(Aria2Status::from)
            })
            .collect())
    }

    /// GIDs em atividade, na fila/pausados e parados que o aria2 ainda guarda.
    pub async fn list_gids(&self) -> Result<(Vec<String>, Vec<String>), RpcError> {
        let keys = json!(["gid"]);
        let pick = |v: Value| -> Vec<String> {
            v.as_array()
                .map(|a| a.iter().filter_map(|x| x.get("gid")?.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        let mut live = pick(self.call("aria2.tellActive", vec![keys.clone()]).await?);
        live.extend(pick(self.call("aria2.tellWaiting", vec![json!(0), json!(10_000), keys.clone()]).await?));
        let stopped = pick(self.call("aria2.tellStopped", vec![json!(0), json!(10_000), keys]).await?);
        Ok((live, stopped))
    }

    /// Move na fila de espera. `how`: POS_SET | POS_CUR | POS_END.
    pub async fn change_position(&self, gid: &str, pos: i64, how: &str) -> Result<(), RpcError> {
        self.call("aria2.changePosition", vec![json!(gid), json!(pos), json!(how)]).await.map(drop)
    }

    pub async fn pause(&self, gid: &str) -> Result<(), RpcError> {
        self.call("aria2.forcePause", vec![json!(gid)]).await.map(drop)
    }

    pub async fn unpause(&self, gid: &str) -> Result<(), RpcError> {
        self.call("aria2.unpause", vec![json!(gid)]).await.map(drop)
    }

    pub async fn remove(&self, gid: &str) -> Result<(), RpcError> {
        self.call("aria2.forceRemove", vec![json!(gid)]).await.map(drop)
    }

    /// Tira um download parado (concluído, com erro ou removido) da memória do aria2.
    pub async fn forget(&self, gid: &str) -> Result<(), RpcError> {
        self.call("aria2.removeDownloadResult", vec![json!(gid)]).await.map(drop)
    }

    pub async fn change_global(&self, options: Map<String, Value>) -> Result<(), RpcError> {
        self.call("aria2.changeGlobalOption", vec![Value::Object(options)]).await.map(drop)
    }

    pub async fn change_option(&self, gid: &str, options: Map<String, Value>) -> Result<(), RpcError> {
        self.call("aria2.changeOption", vec![json!(gid), Value::Object(options)]).await.map(drop)
    }

    pub async fn save_session(&self) -> Result<(), RpcError> {
        self.call("aria2.saveSession", vec![]).await.map(drop)
    }

    pub async fn shutdown(&self) -> Result<(), RpcError> {
        self.call("aria2.shutdown", vec![]).await.map(drop)
    }
}
