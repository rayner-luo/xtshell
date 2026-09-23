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
        .setup(move |app| {
            // 原生弹出（select 下拉等）默认浅色，与默认主题一致；
            // 深色用户由前端加载后经 set_native_theme 切换。
            let dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("xtshell-fallback"));
            let store = match profile::ProfileStore::open(&dir) {
                Ok(store) => store,
                Err(e) => {
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
                if let Err(e) = mcp::serve_mcp(mgr2, profiles2, handle, 46543).await {
                    eprintln!("mcp server failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::close_session,
            commands::list_sessions,
            commands::send_input,
            commands::resize_session,
            commands::get_session_output,
            commands::reconnect_session,
            commands::list_serial_ports,
            commands::set_session_log,
            commands::set_native_theme,
            commands::list_profiles,
            commands::save_profile,
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
