use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;

use crate::error::{AppError, AppResult};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

pub fn load_or_create_key(path: &std::path::Path) -> AppResult<[u8; KEY_LEN]> {
    if let Some(bytes) = std::fs::read(path).ok().filter(|b| b.len() == KEY_LEN) {
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, key).map_err(AppError::Io)?;
    Ok(key)
}

pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_bytes())
        .map_err(|_| AppError::Connection("encrypt failed".into()))?;
    let mut packed = Vec::with_capacity(NONCE_LEN + ct.len());
    packed.extend_from_slice(&nonce_bytes);
    packed.extend_from_slice(&ct);
    Ok(B64.encode(packed))
}

pub fn decrypt(key: &[u8; KEY_LEN], encoded: &str) -> AppResult<String> {
    let packed = B64
        .decode(encoded)
        .map_err(|_| AppError::Connection("bad ciphertext encoding".into()))?;
    if packed.len() < NONCE_LEN {
        return Err(AppError::Connection("ciphertext too short".into()));
    }
    let (nonce, ct) = packed.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| AppError::Connection("decrypt failed".into()))?;
    String::from_utf8(pt).map_err(|_| AppError::Connection("decrypt not utf8".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = [7u8; KEY_LEN];
        let enc = encrypt(&key, "p@ss w?rd!").unwrap();
        assert_ne!(enc, "p@ss w?rd!");
        assert_eq!(decrypt(&key, &enc).unwrap(), "p@ss w?rd!");
    }

    #[test]
    fn unique_nonce_same_plaintext() {
        let key = [9u8; KEY_LEN];
        let e1 = encrypt(&key, "same").unwrap();
        let e2 = encrypt(&key, "same").unwrap();
        assert_ne!(e1, e2);
        assert_eq!(decrypt(&key, &e1).unwrap(), "same");
        assert_eq!(decrypt(&key, &e2).unwrap(), "same");
    }

    #[test]
    fn wrong_key_fails() {
        let enc = encrypt(&[1u8; KEY_LEN], "secret").unwrap();
        assert!(decrypt(&[2u8; KEY_LEN], &enc).is_err());
    }

    #[test]
    fn key_file_roundtrip() {
        let dir = std::env::temp_dir().join(format!("xtshell-key-test-{}", std::process::id()));
        let path = dir.join("secret.key");
        let k1 = load_or_create_key(&path).unwrap();
        let k2 = load_or_create_key(&path).unwrap();
        assert_eq!(k1, k2);
        let k3 = load_or_create_key(&dir.join("other.key")).unwrap();
        assert_ne!(k1, k3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
