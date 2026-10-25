//! 本地凭据保险库：账号 Cookie 只以 AES-256-GCM 密文落盘。
//!
//! ## 威胁模型
//!
//! 目标是「应用数据目录被同步盘、备份工具或其它程序拷走后，无法直接读出 Cookie」。
//!
//! - 默认：随机 32 字节主密钥写在 `master.key`（Unix 上 `0600`），凭据写在 `vault.bin`；
//! - 可选 PIN：主密钥改用 Argon2id 从 PIN 派生的 KEK 包裹，此时没有 PIN 根本解不开。
//!
//! 这**不是**对抗「已取得本机管理员权限的攻击者」的方案。需要更强隔离时，
//! 应把 `master.key` 交给系统密钥库（Windows DPAPI / Android Keystore）托管。
//!
//! ## 文件格式
//!
//! ```text
//! master.key  = 32 字节原始主密钥
//!             | "MPIN" + salt(16) + nonce(12) + AES-GCM(主密钥)
//! vault.bin   = "MELODEX-VAULT1" + nonce(12) + AES-GCM(JSON)
//! ```
//!
//! AAD 不参与绑定：文件整体被替换时解密会失败（GCM tag 校验），已足够。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::Argon2;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::task::now_secs;

const MASTER_FILE: &str = "master.key";
const VAULT_FILE: &str = "vault.bin";
const MAGIC: &[u8] = b"MELODEX-VAULT1";
const PIN_MAGIC: &[u8] = b"MPIN";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
/// GCM 认证标签长度
const TAG_LEN: usize = 16;
/// PIN 最少位数（太短的 PIN 在 Argon2 之外没有意义）
pub const MIN_PIN_LEN: usize = 6;

/// 一条账号凭据。`cookie` 是请求音源接口时要带的完整 Cookie 串。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub cookie: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub saved_at: i64,
}

/// 供界面展示的账号摘要。**绝不包含 Cookie 明文**。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub source: String,
    pub nickname: Option<String>,
    pub saved_at: i64,
    /// Cookie 里包含哪些键（如 `MUSIC_U` / `__csrf`），用于让用户确认登录态存在
    pub keys: Vec<String>,
}

impl AccountInfo {
    pub fn of(source: &str, account: &Account) -> Self {
        Self {
            source: source.to_string(),
            nickname: account.nickname.clone(),
            saved_at: account.saved_at,
            keys: crate::cookie::names(&account.cookie),
        }
    }
}

impl Account {
    pub fn new(cookie: impl Into<String>, nickname: Option<String>) -> Self {
        Self {
            cookie: cookie.into(),
            nickname,
            saved_at: now_secs(),
        }
    }
}

/// 保险库内容。键是音源 id（`netease` / `qq` / `kuwo`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultData {
    #[serde(default)]
    pub accounts: BTreeMap<String, Account>,
}

impl VaultData {
    pub fn cookie(&self, source: &str) -> Option<&str> {
        self.accounts
            .get(source)
            .map(|account| account.cookie.as_str())
    }

    pub fn set(&mut self, source: &str, account: Account) {
        self.accounts.insert(source.to_string(), account);
    }

    pub fn remove(&mut self, source: &str) -> bool {
        self.accounts.remove(source).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// 账号摘要列表，按音源 id 排序。
    pub fn infos(&self) -> Vec<AccountInfo> {
        self.accounts
            .iter()
            .map(|(source, account)| AccountInfo::of(source, account))
            .collect()
    }
}

/// 打开保险库的结果：要么直接可用，要么需要用户输入 PIN。
pub enum Opened {
    Ready(Vault),
    NeedPin,
}

/// 已解锁的保险库。持有主密钥，可读写 `vault.bin`。
#[derive(Clone)]
pub struct Vault {
    dir: PathBuf,
    master: [u8; KEY_LEN],
}

/// 手写 Debug：主密钥必须遮蔽，避免被日志或 panic 信息带出去。
impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("dir", &self.dir)
            .field("master", &"<redacted>")
            .finish()
    }
}

impl Vault {
    /// 打开保险库；不存在则创建（生成随机主密钥）。
    pub fn open(dir: &Path) -> Result<Opened> {
        let path = dir.join(MASTER_FILE);
        let raw = match fs::read(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Opened::Ready(Self::create(dir)?))
            }
            Err(err) => return Err(err.into()),
        };
        if raw.starts_with(PIN_MAGIC) {
            return Ok(Opened::NeedPin);
        }
        if raw.len() != KEY_LEN {
            return Err(Error::Invalid("凭据库主密钥文件已损坏".into()));
        }
        let mut master = [0u8; KEY_LEN];
        master.copy_from_slice(&raw);
        Ok(Opened::Ready(Self {
            dir: dir.to_path_buf(),
            master,
        }))
    }

    /// 用 PIN 解锁。
    pub fn open_with_pin(dir: &Path, pin: &str) -> Result<Self> {
        let raw = fs::read(dir.join(MASTER_FILE))?;
        if !raw.starts_with(PIN_MAGIC) {
            return Err(Error::Invalid("凭据库未启用 PIN".into()));
        }
        let body = &raw[PIN_MAGIC.len()..];
        if body.len() < SALT_LEN + NONCE_LEN + KEY_LEN + TAG_LEN {
            return Err(Error::Invalid("凭据库主密钥文件已损坏".into()));
        }
        let (salt, sealed) = body.split_at(SALT_LEN);
        let kek = derive_key(pin.trim(), salt)?;
        let plain = open_sealed(&kek, sealed).map_err(|_| Error::Invalid("PIN 不正确".into()))?;
        if plain.len() != KEY_LEN {
            return Err(Error::Invalid("凭据库主密钥长度异常".into()));
        }
        let mut master = [0u8; KEY_LEN];
        master.copy_from_slice(&plain);
        Ok(Self {
            dir: dir.to_path_buf(),
            master,
        })
    }

    /// 首次创建：生成随机主密钥并落盘。
    fn create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let master = random_bytes::<KEY_LEN>()?;
        write_private(&dir.join(MASTER_FILE), &master)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            master,
        })
    }

    /// 是否启用了 PIN 保护。
    pub fn is_pin_protected(dir: &Path) -> bool {
        fs::read(dir.join(MASTER_FILE))
            .map(|raw| raw.starts_with(PIN_MAGIC))
            .unwrap_or(false)
    }

    /// 保险库是否已存在（首次启动时为 false）。
    pub fn exists(dir: &Path) -> bool {
        dir.join(MASTER_FILE).exists()
    }

    /// 设置 / 取消 PIN。设置为 `None` 会退化为「随机主密钥 + 文件权限」。
    pub fn set_pin(&self, pin: Option<&str>) -> Result<()> {
        let path = self.dir.join(MASTER_FILE);
        match pin {
            Some(pin) => {
                let pin = pin.trim();
                if pin.chars().count() < MIN_PIN_LEN {
                    return Err(Error::Invalid(format!("PIN 至少 {MIN_PIN_LEN} 位")));
                }
                let salt = random_bytes::<SALT_LEN>()?;
                let kek = derive_key(pin, &salt)?;
                let mut out = PIN_MAGIC.to_vec();
                out.extend_from_slice(&salt);
                out.extend_from_slice(&seal(&kek, &self.master)?);
                write_private(&path, &out)
            }
            None => write_private(&path, &self.master),
        }
    }

    /// 读取凭据。文件不存在时返回空数据。
    pub fn load(&self) -> Result<VaultData> {
        let path = self.dir.join(VAULT_FILE);
        let raw = match fs::read(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(VaultData::default())
            }
            Err(err) => return Err(err.into()),
        };
        if !raw.starts_with(MAGIC) {
            return Err(Error::Invalid("凭据库格式无法识别".into()));
        }
        let plain = open_sealed(&self.master, &raw[MAGIC.len()..])?;
        serde_json::from_slice(&plain)
            .map_err(|err| Error::decode(format!("凭据库解析失败：{err}")))
    }

    /// 覆盖写入凭据。
    pub fn save(&self, data: &VaultData) -> Result<()> {
        let plain = serde_json::to_vec(data)
            .map_err(|err| Error::decode(format!("凭据序列化失败：{err}")))?;
        let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + plain.len() + TAG_LEN);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&seal(&self.master, &plain)?);
        write_private(&self.dir.join(VAULT_FILE), &out)
    }
}

fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf)
        .map_err(|err| Error::Invalid(format!("随机数生成失败：{err}")))?;
    Ok(buf)
}

fn cipher(key: &[u8; KEY_LEN]) -> Aes256Gcm {
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key))
}

/// 加密并前置 nonce，返回 `nonce || ciphertext || tag`。
fn seal(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<Vec<u8>> {
    let nonce_bytes = random_bytes::<NONCE_LEN>()?;
    let body = cipher(key)
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| Error::Invalid("加密失败".into()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + body.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&body);
    Ok(out)
}

fn open_sealed(key: &[u8; KEY_LEN], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < NONCE_LEN + TAG_LEN {
        return Err(Error::Invalid("密文长度异常".into()));
    }
    let (nonce_bytes, body) = sealed.split_at(NONCE_LEN);
    cipher(key)
        .decrypt(Nonce::from_slice(nonce_bytes), body)
        .map_err(|_| Error::Invalid("解密失败：密钥不匹配或数据被篡改".into()))
}

/// 用 Argon2id 从 PIN 派生 32 字节 KEK。
fn derive_key(pin: &str, salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    let mut out = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(pin.as_bytes(), salt, &mut out)
        .map_err(|err| Error::Invalid(format!("密钥派生失败：{err}")))?;
    Ok(out)
}

/// 写入敏感文件：先写临时文件再原子替换，Unix 上收权限到 `0600`。
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("melodex-vault-{tag}-{}", crate::task::make_id(tag)));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn round_trips_accounts() {
        let dir = temp_dir("roundtrip");
        let vault = match Vault::open(&dir).unwrap() {
            Opened::Ready(vault) => vault,
            Opened::NeedPin => panic!("新库不应要求 PIN"),
        };
        let mut data = VaultData::default();
        data.set(
            "netease",
            Account::new("MUSIC_U=abc; __csrf=def", Some("小明".into())),
        );
        vault.save(&data).unwrap();

        let loaded = vault.load().unwrap();
        assert_eq!(loaded.cookie("netease"), Some("MUSIC_U=abc; __csrf=def"));
        assert_eq!(loaded.accounts["netease"].nickname.as_deref(), Some("小明"));

        let _ = fs::remove_dir_all(&dir);
    }

    /// 落盘内容不能出现明文 Cookie —— 这是"本地加密"的最低验收标准。
    #[test]
    fn file_contains_no_plaintext() {
        let dir = temp_dir("plaintext");
        let vault = match Vault::open(&dir).unwrap() {
            Opened::Ready(vault) => vault,
            Opened::NeedPin => unreachable!(),
        };
        let mut data = VaultData::default();
        data.set(
            "qq",
            Account::new("uin=10086; qm_keystore=SECRET-TICKET", None),
        );
        vault.save(&data).unwrap();

        let raw = fs::read(dir.join(VAULT_FILE)).unwrap();
        assert!(!raw.windows(15).any(|w| w == b"MUSIC_U=abc"));
        assert!(!raw.windows(13).any(|w| w == b"SECRET-TICKET"));
        assert!(!String::from_utf8_lossy(&raw).contains("SECRET"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pin_protects_master_key() {
        let dir = temp_dir("pin");
        let vault = match Vault::open(&dir).unwrap() {
            Opened::Ready(vault) => vault,
            Opened::NeedPin => unreachable!(),
        };
        vault.set_pin(Some("135790")).unwrap();
        assert!(Vault::is_pin_protected(&dir));
        assert!(matches!(Vault::open(&dir).unwrap(), Opened::NeedPin));

        let err = Vault::open_with_pin(&dir, "000000").unwrap_err();
        assert!(err.to_string().contains("PIN 不正确"));

        let unlocked = Vault::open_with_pin(&dir, "135790").unwrap();
        let mut data = VaultData::default();
        data.set("kuwo", Account::new("kw_token=xyz", None));
        unlocked.save(&data).unwrap();
        assert_eq!(
            unlocked.load().unwrap().cookie("kuwo"),
            Some("kw_token=xyz")
        );

        // 取消 PIN 后回到随机主密钥模式
        unlocked.set_pin(None).unwrap();
        assert!(!Vault::is_pin_protected(&dir));
        assert!(matches!(Vault::open(&dir).unwrap(), Opened::Ready(_)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_short_pin() {
        let dir = temp_dir("shortpin");
        let vault = match Vault::open(&dir).unwrap() {
            Opened::Ready(vault) => vault,
            Opened::NeedPin => unreachable!(),
        };
        assert!(vault.set_pin(Some("123")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    /// 篡改密文必须解密失败，而不是返回半截数据。
    #[test]
    fn detects_tampering() {
        let dir = temp_dir("tamper");
        let vault = match Vault::open(&dir).unwrap() {
            Opened::Ready(vault) => vault,
            Opened::NeedPin => unreachable!(),
        };
        let mut data = VaultData::default();
        data.set("netease", Account::new("MUSIC_U=abc", None));
        vault.save(&data).unwrap();

        let path = dir.join(VAULT_FILE);
        let mut raw = fs::read(&path).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0x01;
        fs::write(&path, &raw).unwrap();

        assert!(vault.load().is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_vault_reads_empty() {
        let dir = temp_dir("missing");
        let vault = match Vault::open(&dir).unwrap() {
            Opened::Ready(vault) => vault,
            Opened::NeedPin => unreachable!(),
        };
        assert!(vault.load().unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
