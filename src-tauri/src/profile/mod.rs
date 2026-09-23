pub mod crypto;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::{AppError, AppResult};
use crate::session::manager::SessionConfig;
use crypto::{decrypt, encrypt, load_or_create_key};

pub const MAX_FOLDER_DEPTH: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileFolder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "snake_case")]
pub struct TerminalAppearance {
    pub font_family: Option<String>,
    pub font_size: Option<u16>,
    #[serde(default)]
    pub encoding: Option<String>,
    pub color_scheme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSession {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub folder: Option<String>,
    pub protocol: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password_enc: Option<String>,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub private_key_passphrase_enc: Option<String>,
    /// 认证类型："password" | "key"；旧数据在加载时按 private_key 是否存在推断
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(default)]
    pub serial_port: Option<String>,
    #[serde(default)]
    pub baud_rate: Option<u32>,
    #[serde(default)]
    pub data_bits: Option<u8>,
    #[serde(default)]
    pub stop_bits: Option<u8>,
    #[serde(default)]
    pub parity: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub terminal_appearance: Option<TerminalAppearance>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProfileFile {
    pub version: u32,
    #[serde(default)]
    pub folders: Vec<ProfileFolder>,
    #[serde(default)]
    pub sessions: Vec<ProfileSession>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileSessionView {
    pub id: String,
    pub name: String,
    pub folder: Option<String>,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub private_key: Option<String>,
    pub auth_mode: Option<String>,
    pub serial_port: Option<String>,
    pub baud_rate: Option<u32>,
    pub data_bits: Option<u8>,
    pub stop_bits: Option<u8>,
    pub parity: Option<String>,
    pub encoding: Option<String>,
    #[serde(default)]
    pub terminal_appearance: Option<TerminalAppearance>,
    pub created_at: String,
    pub has_password: bool,
    pub has_passphrase: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfilesTree {
    pub folders: Vec<ProfileFolder>,
    pub sessions: Vec<ProfileSessionView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SaveProfileRequest {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub folder: Option<String>,
    pub protocol: String,
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub remember_password: bool,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub private_key_passphrase: Option<String>,
    #[serde(default)]
    pub serial_port: Option<String>,
    #[serde(default)]
    pub baud_rate: Option<u32>,
    #[serde(default)]
    pub data_bits: Option<u8>,
    #[serde(default)]
    pub stop_bits: Option<u8>,
    #[serde(default)]
    pub parity: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub terminal_appearance: Option<TerminalAppearance>,
}

pub struct ProfileStore {
    path: PathBuf,
    key: [u8; 32],
    file: ProfileFile,
}

impl ProfileStore {
    pub fn open(dir: &std::path::Path) -> AppResult<Self> {
        let path = dir.join("sessions.yaml");
        let mut file = match std::fs::read_to_string(&path) {
            Ok(text) => serde_yaml::from_str(&text).unwrap_or_default(),
            Err(_) => ProfileFile { version: 1, ..Default::default() },
        };
        // 迁移：旧数据无 auth_mode 时按 private_key 是否存在推断
        let mut migrated = false;
        for s in &mut file.sessions {
            if s.auth_mode.is_none() {
                s.auth_mode = Some(
                    if s.private_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false) {
                        "key".into()
                    } else {
                        "password".into()
                    },
                );
                migrated = true;
            }
        }
        let key = load_or_create_key(&dir.join("secret.key"))?;
        let store = ProfileStore { path, key, file };
        if migrated {
            let _ = store.save();
        }
        Ok(store)
    }

    fn save(&self) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let text = serde_yaml::to_string(&self.file)
            .map_err(|e| AppError::Connection(format!("yaml: {e}")))?;
        std::fs::write(&self.path, text).map_err(AppError::Io)
    }

    pub fn tree(&self) -> ProfilesTree {
        ProfilesTree {
            folders: self.file.folders.clone(),
            sessions: self
                .file
                .sessions
                .iter()
                .map(|s| ProfileSessionView {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    folder: s.folder.clone(),
                    protocol: s.protocol.clone(),
                    host: s.host.clone(),
                    port: s.port,
                    username: s.username.clone(),
                    private_key: s.private_key.clone(),
                    auth_mode: s.auth_mode.clone(),
                    serial_port: s.serial_port.clone(),
                    baud_rate: s.baud_rate,
                    data_bits: s.data_bits,
                    stop_bits: s.stop_bits,
            parity: s.parity.clone(),
            encoding: s.encoding.clone(),
            terminal_appearance: s.terminal_appearance.clone(),
            created_at: s.created_at.clone(),
                    has_password: s.password_enc.is_some(),
                    has_passphrase: s.private_key_passphrase_enc.is_some(),
                })
                .collect(),
        }
    }

    fn folder_exists(&self, id: &str) -> bool {
        id.is_empty() || self.file.folders.iter().any(|f| f.id == id)
    }

    fn folder_depth(&self, id: &str) -> usize {
        let mut depth = 0;
        let mut cur = Some(id.to_string());
        while let Some(cid) = cur {
            depth += 1;
            cur = self
                .file
                .folders
                .iter()
                .find(|f| f.id == cid)
                .and_then(|f| f.parent.clone());
        }
        depth
    }

    fn is_descendant(&self, ancestor: &str, candidate: &str) -> bool {
        let mut cur = self
            .file
            .folders
            .iter()
            .find(|f| f.id == candidate)
            .and_then(|f| f.parent.clone());
        while let Some(pid) = cur {
            if pid == ancestor {
                return true;
            }
            cur = self
                .file
                .folders
                .iter()
                .find(|f| f.id == pid)
                .and_then(|f| f.parent.clone());
        }
        false
    }

    fn validate_parent(&self, parent: &Option<String>, moving: Option<&str>) -> AppResult<()> {
        if let Some(pid) = parent {
            if !self.folder_exists(pid) {
                return Err(AppError::Connection(format!("folder not found: {pid}")));
            }
            if pid == "root" {
                return Ok(());
            }
            if let Some(mid) = moving {
                if mid == pid || self.is_descendant(mid, pid) {
                    return Err(AppError::Connection("cannot move folder into itself".into()));
                }
            }
            let depth = self.folder_depth(pid) + 1;
            if depth > MAX_FOLDER_DEPTH {
                return Err(AppError::Connection(format!(
                    "folder nesting exceeds {MAX_FOLDER_DEPTH} levels"
                )));
            }
        }
        Ok(())
    }

    pub fn create_folder(&mut self, name: &str, parent: Option<String>) -> AppResult<String> {
        self.validate_parent(&parent, None)?;
        let id = uuid::Uuid::new_v4().to_string();
        self.file.folders.push(ProfileFolder {
            id: id.clone(),
            name: name.to_string(),
            parent: normalize_root(parent),
        });
        self.save()?;
        Ok(id)
    }

    pub fn rename_folder(&mut self, id: &str, name: &str) -> AppResult<()> {
        let f = self
            .file
            .folders
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| AppError::Connection(format!("folder not found: {id}")))?;
        f.name = name.to_string();
        self.save()
    }

    pub fn move_folder(&mut self, id: &str, parent: Option<String>) -> AppResult<()> {
        self.validate_parent(&parent, Some(id))?;
        let f = self
            .file
            .folders
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| AppError::Connection(format!("folder not found: {id}")))?;
        f.parent = normalize_root(parent);
        self.save()
    }

    pub fn delete_folder(&mut self, id: &str) -> AppResult<()> {
        if self.file.sessions.iter().any(|s| s.folder.as_deref() == Some(id)) {
            return Err(AppError::Connection("folder not empty (has sessions)".into()));
        }
        if self.file.folders.iter().any(|f| f.parent.as_deref() == Some(id)) {
            return Err(AppError::Connection("folder not empty (has subfolders)".into()));
        }
        let before = self.file.folders.len();
        self.file.folders.retain(|f| f.id != id);
        if self.file.folders.len() == before {
            return Err(AppError::Connection(format!("folder not found: {id}")));
        }
        self.save()
    }

    pub fn upsert_session(
        &mut self,
        req: SaveProfileRequest,
    ) -> AppResult<String> {
        if let Some(fid) = req.folder.as_deref() {
            if !self.folder_exists(fid) {
                return Err(AppError::Connection(format!("folder not found: {fid}")));
            }
        }
        let id = req.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let now = chrono::Utc::now().to_rfc3339();
        let existing = self.file.sessions.iter_mut().find(|s| s.id == id);

        let password_enc = if req.remember_password {
            req.password
                .as_deref()
                .filter(|p| !p.is_empty())
                .map(|p| encrypt(&self.key, p))
                .transpose()?
                .or_else(|| existing.as_ref().and_then(|s| s.password_enc.clone()))
        } else {
            None
        };
        let passphrase_enc = if req.remember_password {
            req.private_key_passphrase
                .as_deref()
                .filter(|p| !p.is_empty())
                .map(|p| encrypt(&self.key, p))
                .transpose()?
                .or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|s| s.private_key_passphrase_enc.clone())
                })
        } else {
            None
        };

        let created_at = match &existing {
            Some(s) => s.created_at.clone(),
            None => now,
        };

        let new_entry = ProfileSession {
            id: id.clone(),
            name: req.name,
            folder: req.folder.map(|f| if f == "root" { String::new() } else { f })
                .filter(|f| !f.is_empty()),
            protocol: req.protocol,
            host: req.host,
            port: req.port,
            username: req.username,
            password_enc,
            private_key: req.private_key,
            private_key_passphrase_enc: passphrase_enc,
            auth_mode: req.auth_mode,
            serial_port: req.serial_port,
            baud_rate: req.baud_rate,
            data_bits: req.data_bits,
            stop_bits: req.stop_bits,
            parity: req.parity,
            encoding: req.encoding,
            terminal_appearance: req.terminal_appearance,
            created_at,
        };

        if let Some(slot) = existing {
            *slot = new_entry;
        } else {
            self.file.sessions.push(new_entry);
        }
        self.save()?;
        Ok(id)
    }

    pub fn delete_session(&mut self, id: &str) -> AppResult<()> {
        let before = self.file.sessions.len();
        self.file.sessions.retain(|s| s.id != id);
        if self.file.sessions.len() == before {
            return Err(AppError::Connection(format!("profile not found: {id}")));
        }
        self.save()
    }

    pub fn move_session(&mut self, id: &str, folder: Option<String>) -> AppResult<()> {
        self.validate_parent(&folder, None)?;
        let s = self
            .file
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::Connection(format!("profile not found: {id}")))?;
        s.folder = folder.map(|f| if f == "root" { String::new() } else { f })
            .filter(|f| !f.is_empty());
        self.save()
    }

    pub fn set_terminal_appearance(
        &mut self,
        id: &str,
        appearance: TerminalAppearance,
    ) -> AppResult<()> {
        let s = self
            .file
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::SessionNotFound(id.to_string()))?;
        s.terminal_appearance = Some(appearance);
        self.save()
    }

    pub fn build_session_config(&self, id: &str) -> AppResult<SessionConfig> {
        let p = self
            .file
            .sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::SessionNotFound(id.to_string()))?;
        let password = p
            .password_enc
            .as_deref()
            .map(|enc| decrypt(&self.key, enc))
            .transpose()?;
        let passphrase = p
            .private_key_passphrase_enc
            .as_deref()
            .map(|enc| decrypt(&self.key, enc))
            .transpose()?;
        Ok(SessionConfig {
            protocol: p.protocol.clone(),
            host: p.host.clone(),
            port: p.port,
            username: p.username.clone(),
            password,
            private_key: p.private_key.clone(),
            private_key_passphrase: passphrase,
            serial_port: p.serial_port.clone(),
            baud_rate: p.baud_rate,
            data_bits: p.data_bits,
            stop_bits: p.stop_bits,
            parity: p.parity.clone(),
        })
    }

    /// 连接时收集的认证信息（连接前弹框）部分更新：只写提供的字段
    pub fn set_profile_secret(
        &mut self,
        id: &str,
        password: Option<String>,
        passphrase: Option<String>,
        private_key: Option<String>,
    ) -> AppResult<()> {
        let p = self
            .file
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::SessionNotFound(id.to_string()))?;
        if let Some(pw) = password.as_deref().filter(|p| !p.is_empty()) {
            p.password_enc = Some(encrypt(&self.key, pw)?);
        }
        if let Some(pp) = passphrase.as_deref().filter(|p| !p.is_empty()) {
            p.private_key_passphrase_enc = Some(encrypt(&self.key, pp)?);
        }
        if let Some(k) = private_key.as_deref().filter(|k| !k.is_empty()) {
            p.private_key = Some(k.to_string());
        }
        self.save()
    }
}

fn normalize_root(parent: Option<String>) -> Option<String> {
    parent.filter(|p| !p.is_empty() && p != "root")
}

fn _normalize_helper() {}

pub type SharedProfiles = Arc<Mutex<ProfileStore>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> ProfileStore {
        // 并行测试各自拿独立目录：共用目录时一个测试 remove_dir_all 会删掉
        // 另一个测试正在用的 secret.key，导致解密偶发失败。
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "xtshell-profile-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        ProfileStore::open(&dir).unwrap()
    }

    #[test]
    fn yaml_roundtrip_persists() {
        let dir = std::env::temp_dir().join(format!("xtshell-yaml-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = ProfileStore::open(&dir).unwrap();
        let fid = s.create_folder("prod", None).unwrap();
        s.upsert_session(SaveProfileRequest {
            id: None,
            name: "web-01".into(),
            folder: Some(fid.clone()),
            protocol: "ssh".into(),
            host: Some("1.2.3.4".into()),
            port: Some(22),
            username: Some("u".into()),
            password: Some("secret".into()),
            remember_password: true,
            private_key: Some("C:\\k".into()),
            private_key_passphrase: None,
            encoding: Some("gbk".into()),
            ..Default::default()
        })
        .unwrap();

        let s2 = ProfileStore::open(&dir).unwrap();
        assert_eq!(s2.file.folders.len(), 1);
        assert_eq!(s2.file.sessions.len(), 1);
        let p = &s2.file.sessions[0];
        assert_eq!(p.folder.as_deref(), Some(fid.as_str()));
        assert_eq!(p.encoding.as_deref(), Some("gbk"));
        assert!(p.password_enc.is_some());
        assert_ne!(p.password_enc.as_deref(), Some("secret"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nesting_depth_limit() {
        let mut s = store();
        let mut parent: Option<String> = None;
        for i in 0..MAX_FOLDER_DEPTH {
            parent = Some(s.create_folder(&format!("d{i}"), parent.clone()).unwrap());
        }
        assert_eq!(s.file.folders.len(), MAX_FOLDER_DEPTH);
        assert!(s.create_folder("too-deep", parent.clone()).is_err());
    }

    #[test]
    fn move_folder_cycle_rejected() {
        let mut s = store();
        let a = s.create_folder("a", None).unwrap();
        let b = s.create_folder("b", Some(a.clone())).unwrap();
        assert!(s.move_folder(&a, Some(b.clone())).is_err());
        s.move_folder(&b, None).unwrap();
        assert_eq!(
            s.file.folders.iter().find(|f| f.id == b).unwrap().parent,
            None
        );
    }

    #[test]
    fn delete_rules() {
        let mut s = store();
        let a = s.create_folder("a", None).unwrap();
        let b = s.create_folder("b", Some(a.clone())).unwrap();
        assert!(s.delete_folder(&a).is_err());
        assert!(s.delete_folder(&b).is_ok());
        assert!(s.delete_folder(&a).is_ok());
        assert!(s.delete_folder(&a).is_err());
    }

    #[test]
    fn password_not_remembered_stays_null() {
        let mut s = store();
        s.upsert_session(SaveProfileRequest {
            id: None,
            name: "n".into(),
            folder: None,
            protocol: "ssh".into(),
            host: Some("h".into()),
            port: None,
            username: None,
            password: Some("p".into()),
            remember_password: false,
            private_key: None,
            private_key_passphrase: None,
            ..Default::default()
        })
        .unwrap();
        assert!(s.file.sessions[0].password_enc.is_none());
    }

    #[test]
    fn build_session_decrypts_password() {
        let mut s = store();
        let id = s
            .upsert_session(SaveProfileRequest {
                id: None,
                name: "n".into(),
                folder: None,
                protocol: "ssh".into(),
                host: Some("h".into()),
                port: Some(2222),
                username: Some("u".into()),
                password: Some("pw".into()),
                remember_password: true,
                private_key: None,
                private_key_passphrase: None,
                ..Default::default()
            })
            .unwrap();
        let cfg = s.build_session_config(&id).unwrap();
        assert_eq!(cfg.password.as_deref(), Some("pw"));
        assert_eq!(cfg.port, Some(2222));
    }
}
