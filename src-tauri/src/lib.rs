mod applog;
mod connector;
mod commands;
mod error;
mod mcp;
mod profile;
mod protocol;
mod session;
mod sftp;
mod zmodem;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;

use session::manager::SharedManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mgr: SharedManager = Arc::new(Mutex::new(session::manager::SessionManager::new(64 * 1024)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(mgr.clone())
        .manage(zmodem::ZmodemRegistry::default())
        .manage(sftp::SftpRegistry::default())
        .manage(Arc::new(protocol::ssh::HostKeyGate::default()))
        .setup(move |app| {
            // 原生弹出（select 下拉等）默认浅色，与默认主题一致；
            // 深色用户由前端加载后经 set_native_theme 切换。
            let dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("xtshell-fallback"));
            // 启动日志先就位：后续初始化结果、MCP 绑定失败等才有地方落盘，
            // 用户可在「日志」菜单直接看到（GUI 程序没有控制台）。
            applog::init(&dir);
            applog::info(&format!(
                "xtshell {} 启动（{} {}）",
                app.package_info().version,
                std::env::consts::OS,
                std::env::consts::ARCH
            ));
            applog::info(&format!("数据目录: {}", dir.display()));

            let store = match profile::ProfileStore::open(&dir) {
                Ok(store) => {
                    applog::info("会话配置加载完成");
                    store
                }
                Err(e) => {
                    applog::error(&format!("会话配置加载失败: {e}（改用临时目录）"));
                    eprintln!("profile store init failed: {e}");
                    profile::ProfileStore::open(&std::env::temp_dir().join("xtshell-fallback"))
                        .expect("fallback profile store")
                }
            };
            app.manage(Arc::new(Mutex::new(store)) as profile::SharedProfiles);

            let mgr2 = mgr.clone();
            let profiles2 = app
                .state::<profile::SharedProfiles>()
                .inner()
                .clone();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mcp::serve_mcp(mgr2, profiles2, handle, 9567).await {
                    applog::error(&format!("MCP 服务启动失败: {e}"));
                    eprintln!("mcp server failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::close_session,
            commands::list_sessions,
            commands::ssh_hostkey_decision,
            commands::send_input,
            commands::resize_session,
            commands::get_session_output,
            commands::reconnect_session,
            commands::list_serial_ports,
            commands::set_session_log,
            commands::set_native_theme,
            commands::read_app_log,
            commands::clear_app_log,
            commands::list_profiles,
            commands::save_profile,
            commands::update_profile_appearance,
            commands::delete_profile,
            commands::launch_profile,
            commands::set_profile_secret,
            commands::create_folder,
            commands::rename_folder,
            commands::delete_folder,
            commands::move_profile,
            commands::move_folder,
            commands::zmodem_start_receive,
            commands::zmodem_send_files,
            commands::zmodem_resolve_conflict,
            commands::zmodem_cancel,
            commands::sftp_connect,
            commands::sftp_disconnect,
            commands::sftp_list,
            commands::sftp_mkdir,
            commands::sftp_remove,
            commands::sftp_rename,
            commands::sftp_check_upload,
            commands::sftp_upload,
            commands::sftp_download,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
