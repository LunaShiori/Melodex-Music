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

/// 按当前运行平台挑出最合适的安装包。
///
/// Windows 优先 NSIS（`.exe`）：它能覆盖安装并自动结束旧进程，是唯一能被
/// 应用自己拉起来、装完就完成升级的格式；`.msi` 留给企业批量分发。
/// Android 只有 `.apk`；其余平台按各自习惯排序。
pub fn pick_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let order: &[&str] = if cfg!(target_os = "windows") {
        &["nsis", "msi", "zip"]
    } else if cfg!(target_os = "android") {
        &["apk", "aab"]
    } else if cfg!(target_os = "macos") {
        &["dmg", "zip"]
    } else {
        &["appimage", "deb", "zip"]
    };
    order
        .iter()
        .find_map(|kind| assets.iter().find(|asset| asset.kind == *kind))
        .or_else(|| assets.first())
}

/// 去掉文件名里的路径部分，防止服务端返回 `../../x.exe` 这类名字。
fn safe_file_name(name: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim()
        .trim_matches('.');
    if base.is_empty() {
        "melodex-update".to_string()
    } else {
        base.to_string()
    }
}

/// 把安装包下载到 `dir`，边下边回报进度，落地前做完整性校验。
///
/// 先写 `.part` 再改名，避免下载中断留下一个「看起来能用」的半截文件。
/// `expected_size` 来自发布清单；服务端给了 `Content-Length` 时以它为准。
#[cfg(feature = "net")]
pub async fn download_asset(
    net: &crate::net::Net,
    url: &str,
    dir: &std::path::Path,
    file_name: &str,
    expected_size: u64,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<std::path::PathBuf> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    tokio::fs::create_dir_all(dir).await?;
    let target = dir.join(safe_file_name(file_name));
    let part = target.with_extension("part");

    let response = net.get(url, &[]).await?;
    let total = response.content_length().unwrap_or(expected_size);
    let mut file = tokio::fs::File::create(&part).await?;
    let mut stream = response.bytes_stream();
    let mut done = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        done += chunk.len() as u64;
        on_progress(done, total);
    }
    file.flush().await?;
    file.sync_all().await.ok();
    drop(file);

    let fail = |reason: String| Error::Source {
        platform: "update".into(),
        reason,
    };
    if done == 0 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(fail("下载到的安装包是空的，请检查网络后重试".into()));
    }
    if total > 0 && done != total {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(fail(format!("安装包下载不完整（{done}/{total} 字节）")));
    }

    // Windows 上目标文件可能正被占用（上一次没退出干净），先删再改名
    if tokio::fs::try_exists(&target).await.unwrap_or(false) {
        let _ = tokio::fs::remove_file(&target).await;
    }
    tokio::fs::rename(&part, &target).await?;
    Ok(target)
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

    #[test]
    fn picks_platform_asset_in_priority_order() {
        let assets = vec![
            ReleaseAsset {
                name: "Melodex_1.0.0_x64_en-US.msi".into(),
                size: 1,
                url: "u1".into(),
                kind: "msi".into(),
            },
            ReleaseAsset {
                name: "Melodex_1.0.0_x64-setup.exe".into(),
                size: 2,
                url: "u2".into(),
                kind: "nsis".into(),
            },
            ReleaseAsset {
                name: "Melodex_1.0.0_universal.apk".into(),
                size: 3,
                url: "u3".into(),
                kind: "apk".into(),
            },
        ];
        let picked = pick_asset(&assets).unwrap();
        if cfg!(target_os = "windows") {
            assert_eq!(picked.kind, "nsis");
        } else if cfg!(target_os = "android") {
            assert_eq!(picked.kind, "apk");
        }
        // 没有偏好类型时退回第一个，不能返回 None 让界面无法下载
        let only = vec![ReleaseAsset {
            name: "x.zip".into(),
            size: 0,
            url: "u".into(),
            kind: "zip".into(),
        }];
        assert!(pick_asset(&only).is_some());
        assert!(pick_asset(&[]).is_none());
    }

    #[test]
    fn file_name_cannot_escape_directory() {
        assert_eq!(safe_file_name("../../evil.exe"), "evil.exe");
        assert_eq!(safe_file_name("C:////Windows////a.exe"), "a.exe");
        assert_eq!(
            safe_file_name("Melodex_0.4.0_x64-setup.exe"),
            "Melodex_0.4.0_x64-setup.exe"
        );
        assert_eq!(safe_file_name(".."), "melodex-update");
    }
}
