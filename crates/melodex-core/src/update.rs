//! 在线更新检查。
//!
//! 这里只做"检查"这一层：拉取发布清单、比较版本号、把结果交给界面。
//! 真正的下载与替换交给用户（点开浏览器下载安装包）或后续接入的
//! `tauri-plugin-updater`，这样核心不需要签名密钥、也不会偷偷改写磁盘。
//!
//! 兼容两种清单格式：
//!  1. GitHub Releases API（`tag_name` / `body` / `assets[].browser_download_url`）—— 默认；
//!  2. 自建极简清单（`version` / `notes` / `url` / `assets[]`）—— 便于镜像分发。

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// 默认发布源：GitHub Releases 的 latest。
/// 换成你自己的仓库即可；也可以在运行时通过 `MELODEX_UPDATE_FEED` 环境变量覆盖。
pub const DEFAULT_FEED: &str = "https://api.github.com/repos/LunaShiori/melodex/releases/latest";

/// 运行时可覆盖的清单地址。
pub fn feed_url() -> String {
    std::env::var("MELODEX_UPDATE_FEED").unwrap_or_else(|_| DEFAULT_FEED.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAsset {
    pub name: String,
    pub size: u64,
    pub url: String,
    /// 安装包类型：nsis / msi / apk / aab / dmg / other，界面据此给不同文案
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub notes: String,
    pub page_url: String,
    pub published_at: String,
    pub assets: Vec<ReleaseAsset>,
}

/// 把 "v0.2.0" / "0.2.0-beta.1" 拆成 (数字段, 预发布标签)
fn split_version(raw: &str) -> (Vec<u64>, Option<String>) {
    let trimmed = raw.trim();
    let trimmed = trimmed.trim_start_matches(['v', 'V']);
    let (core, pre) = match trimmed.split_once('-') {
        Some((core, pre)) => (core, Some(pre.to_ascii_lowercase())),
        None => (trimmed, None),
    };
    let nums = core
        .split('.')
        .map(|seg| {
            let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<u64>().unwrap_or(0)
        })
        .collect();
    (nums, pre)
}

/// 语义化版本比较：数字段逐个比，缺位补 0；同号时正式版 > 预发布版。
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (na, pa) = split_version(a);
    let (nb, pb) = split_version(b);
    for index in 0..na.len().max(nb.len()) {
        let left = na.get(index).copied().unwrap_or(0);
        let right = nb.get(index).copied().unwrap_or(0);
        match left.cmp(&right) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    match (pa, pb) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => left.cmp(&right),
    }
}

fn asset_kind(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for (suffix, kind) in [
        (".exe", "nsis"),
        (".msi", "msi"),
        (".apk", "apk"),
        (".aab", "aab"),
        (".dmg", "dmg"),
        (".appimage", "appimage"),
        (".deb", "deb"),
        (".zip", "zip"),
    ] {
        if lower.ends_with(suffix) {
            return kind.to_string();
        }
    }
    "other".to_string()
}

/// 解析发布清单。`current` 用来判断是否有新版本。
pub fn parse_release(body: &str, current: &str) -> Result<UpdateInfo> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|err| Error::decode(format!("发布清单不是合法 JSON：{err}")))?;

    let obj = value
        .as_object()
        .ok_or_else(|| Error::decode("发布清单顶层不是对象".to_string()))?;

    // GitHub Releases：tag_name / html_url / body / assets[].browser_download_url
    // 自建清单：        version / url / notes / assets[].url
    let latest = obj
        .get("tag_name")
        .or_else(|| obj.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::decode("发布清单缺少 tag_name / version".to_string()))?
        .trim()
        .to_string();

    let notes = obj
        .get("body")
        .or_else(|| obj.get("notes"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let page_url = obj
        .get("html_url")
        .or_else(|| obj.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let published_at = obj
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut assets = Vec::new();
    if let Some(list) = obj.get("assets").and_then(|v| v.as_array()) {
        for item in list {
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let url = item
                .get("browser_download_url")
                .or_else(|| item.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            assets.push(ReleaseAsset {
                name: name.to_string(),
                size: item.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                url: url.to_string(),
                kind: asset_kind(name),
            });
        }
    }

    Ok(UpdateInfo {
        current: current.to_string(),
        available: compare_versions(&latest, current) == Ordering::Greater,
        latest,
        notes,
        page_url,
        published_at,
        assets,
    })
}

/// 联网检查更新。`feed` 传空则用默认地址。
#[cfg(feature = "net")]
pub async fn check(net: &crate::net::Net, feed: &str, current: &str) -> Result<UpdateInfo> {
    let url = if feed.trim().is_empty() {
        feed_url()
    } else {
        feed.trim().to_string()
    };
    let body = net
        .text(
            &url,
            &[
                ("Accept", "application/vnd.github+json"),
                ("Cache-Control", "no-cache"),
            ],
        )
        .await?;
    parse_release(&body, current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering() {
        assert_eq!(compare_versions("0.2.0", "0.1.0"), Ordering::Greater);
        assert_eq!(compare_versions("v0.1.0", "0.1.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.1.10", "0.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.1.0-beta.2", "0.1.0"), Ordering::Less);
        assert_eq!(
            compare_versions("0.1.0-beta.2", "0.1.0-beta.1"),
            Ordering::Greater
        );
    }

    #[test]
    fn parses_github_release() {
        let body = r#"{
            "tag_name": "v0.2.0",
            "body": "更新内容\n- 修复断点续传",
            "html_url": "https://github.com/LunaShiori/melodex/releases/tag/v0.2.0",
            "published_at": "2026-01-01T00:00:00Z",
            "assets": [
                {"name": "Melodex_0.2.0_x64-setup.exe", "size": 4500000, "browser_download_url": "https://example.com/a.exe"},
                {"name": "Melodex_0.2.0_universal.apk", "size": 9000000, "browser_download_url": "https://example.com/b.apk"}
            ]
        }"#;
        let info = parse_release(body, "0.1.0").unwrap();
        assert!(info.available);
        assert_eq!(info.latest, "v0.2.0");
        assert_eq!(info.assets.len(), 2);
        assert_eq!(info.assets[0].kind, "nsis");
        assert_eq!(info.assets[1].kind, "apk");
        assert!(info.notes.contains("断点续传"));
    }

    #[test]
    fn parses_flat_manifest_and_detects_same_version() {
        let body =
            r#"{"version":"0.1.0","notes":"当前已是最新","url":"https://example.com/releases"}"#;
        let info = parse_release(body, "0.1.0").unwrap();
        assert!(!info.available);
        assert!(info.assets.is_empty());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_release("<html>404</html>", "0.1.0").is_err());
        assert!(parse_release("{}", "0.1.0").is_err());
    }
}
