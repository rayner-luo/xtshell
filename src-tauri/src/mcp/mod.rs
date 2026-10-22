use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpService, session::never::NeverSessionManager, tower::StreamableHttpServerConfig,
    },
};
use tauri::{AppHandle, Emitter, Manager};

use crate::session::manager::{SessionConfig, SharedManager};
use crate::profile::SharedProfiles;

#[derive(Clone)]
pub struct McpServer {
    tool_router: ToolRouter<Self>,
    mgr: SharedManager,
    profiles: SharedProfiles,
    app: AppHandle,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateSessionParams {
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub serial_port: Option<String>,
    pub baud_rate: Option<u32>,
    pub data_bits: Option<u8>,
    pub stop_bits: Option<u8>,
    pub parity: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct IdParams {
    pub session_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SendParams {
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConnectProfileParams {
    pub id: String,
    pub password: Option<String>,
    pub passphrase: Option<String>,
    pub private_key: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetOutputParams {
    pub session_id: String,
    pub offset: Option<u64>,
    pub tail: Option<usize>,
}

#[tool_router(router = tool_router)]
impl McpServer {
    fn new(mgr: SharedManager, profiles: SharedProfiles, app: AppHandle) -> Self {
        Self { tool_router: Self::tool_router(), mgr, profiles, app }
    }

    #[tool(description = "Create a new SSH/Telnet/Serial session. Returns JSON {session_id}. protocol: ssh|telnet|serial")]
    async fn create_session(
        &self,
        Parameters(p): Parameters<CreateSessionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let cfg = SessionConfig {
            protocol: p.protocol,
            host: p.host,
            port: p.port,
            username: p.username,
            password: p.password,
            private_key: p.private_key,
            private_key_passphrase: p.private_key_passphrase,
            serial_port: p.serial_port,
            baud_rate: p.baud_rate,
            data_bits: p.data_bits,
            stop_bits: p.stop_bits,
            parity: p.parity,
            // MCP 手工建会话：不关联档案（无 UI 重输流程）
            profile_id: None,
            forwards: Vec::new(),
        };
        let id = self.mgr.lock().await.create(cfg.clone());
        let _ = self.app.emit("sessions-changed", ());
        let mgr2 = self.mgr.clone();
        let id2 = id.clone();
        let app = self.app.clone();
        tauri::async_runtime::spawn(crate::connector::connect_session(mgr2, id2, cfg, app));
        Ok(tool_text(format!("{{\"session_id\":\"{id}\"}}")))
    }

    #[tool(description = "Close and remove a session by id")]
    async fn close_session(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let stream = self
            .mgr
            .lock()
            .await
            .take_for_close(&p.session_id)
            .map_err(map_err)?;
        if let Some(gate) = self.app.try_state::<std::sync::Arc<crate::protocol::ssh::HostKeyGate>>() {
            gate.cancel(&p.session_id);
        }
        if let Some(mut stream) = stream {
            stream.close().await.map_err(map_err)?;
        }
        let _ = self.app.emit("sessions-changed", ());
        Ok(tool_text("closed".into()))
    }

    #[tool(description = "List all sessions as JSON array [{id,protocol,target,state}]")]
    async fn list_sessions(&self) -> Result<CallToolResult, ErrorData> {
        let v = self.mgr.lock().await.list();
        Ok(tool_text(serde_json::to_string(&v).unwrap_or_else(|_| "[]".into())))
    }

    #[tool(description = "List saved session profiles as JSON {folders:[{id,name,parent}],sessions:[{id,name,protocol,host,port,username,auth_mode,has_password,has_passphrase,folder,encoding}]}. Only has_password booleans, never credentials. Use connect_profile to launch one.")]
    async fn list_profiles(&self) -> Result<CallToolResult, ErrorData> {
        let tree = self.profiles.lock().await.tree();
        Ok(tool_text(serde_json::to_string(&tree).unwrap_or_else(|_| "{}".into())))
    }

    #[tool(description = "Launch a saved session profile by id. Stored credentials (remember=true) are used automatically; pass password/passphrase/private_key only as overrides when the profile has none. Returns JSON {session_id}. Then use send_input/get_output on it.")]
    async fn connect_profile(
        &self,
        Parameters(p): Parameters<ConnectProfileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let session_id = crate::commands::launch_session_from_profile(
            self.app.clone(),
            self.profiles.clone(),
            self.mgr.clone(),
            p.id.clone(),
            p.password.clone(),
            p.passphrase.clone(),
            p.private_key.clone(),
        )
        .await
        .map_err(|e| ErrorData::internal_error(e.message, None))?;
        Ok(tool_text(format!("{{\"session_id\":\"{session_id}\"}}")))
    }

    #[tool(description = "Send input text to a session (append \\n yourself to press Enter)")]
    async fn send_input(
        &self,
        Parameters(p): Parameters<SendParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::zmodem::note_input(&p.session_id, &p.text);
        self.mgr
            .lock()
            .await
            .send_input(&p.session_id, p.text.as_bytes())
            .await
            .map_err(map_err)?;
        Ok(tool_text("sent".into()))
    }

    #[tool(description = "Read session output. If tail given: last N bytes as text. Else incremental: returns JSON {output, next_offset}; pass next_offset as offset next time. Poll repeatedly until stable to detect command completion")]
    async fn get_output(
        &self,
        Parameters(p): Parameters<GetOutputParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mgr = self.mgr.lock().await;
        if let Some(t) = p.tail {
            let bytes = mgr.tail_output(&p.session_id, t).map_err(map_err)?;
            return Ok(tool_text(String::from_utf8_lossy(&bytes).to_string()));
        }
        let (out, next) = mgr
            .get_output(&p.session_id, p.offset.unwrap_or(0))
            .map_err(map_err)?;
        Ok(tool_text(
            serde_json::json!({ "output": String::from_utf8_lossy(&out), "next_offset": next })
                .to_string(),
        ))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        info.server_info = Implementation::new("xtshell-mcp", env!("CARGO_PKG_VERSION"));
        info
    }
}

fn tool_text(s: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(s)])
}

fn map_err(e: crate::error::AppError) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

pub async fn serve_mcp(
    mgr: SharedManager,
    profiles: SharedProfiles,
    app: AppHandle,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    // 无状态模式：不要求 Mcp-Session-Id，每个 POST 独立处理（不先 initialize
    // 直接 tools/list 也能工作），对简易 agent 客户端最兼容；工具本身无会话
    // 语义（会话状态在 SessionManager 全局注册表中）。
    // json_response=true：普通请求-响应工具直接回 application/json，
    // 简易客户端无需解析 SSE 流。
    let service = StreamableHttpService::new(
        move || Ok(McpServer::new(mgr.clone(), profiles.clone(), app.clone())),
        NeverSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    crate::applog::info(&format!("MCP 服务已监听 http://127.0.0.1:{port}/mcp"));
    axum::serve(listener, router).await?;
    Ok(())
}
