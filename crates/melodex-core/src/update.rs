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
#[cfg_attr(not(feature = "net"), allow(dead_code))]
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

/// GitHub 加速代理镜像列表。
///
/// 直连 GitHub Releases 在国内经常超时/被重置，导致更新失败。这里按顺序列出
/// 若干把原始 URL 拼到后面的反向代理（`ghproxy` 系 / `moeyy` 系），下载失败时
/// 自动顺延到下一个。它们只是转发，不改动文件内容，因此 `.part` 断点续传仍然
/// 按原始 URL 的字节偏移进行，换源后可以接着下。
///
/// 前缀约定：`{url}` 会被替换成原始的 `browser_download_url`。
#[cfg_attr(not(feature = "net"), allow(dead_code))]
const ACCELERATE_PREFIXES: &[&str] = &[
    "https://mirror.ghproxy.com/",
    "https://ghproxy.net/",
    "https://gh-proxy.com/",
    "https://ghfast.top/",
    "https://gh.api.99988866.xyz/",
];

/// 给某个原始下载地址拼出「加速源优先候选列表」。
///
/// 只有 GitHub 域名的地址才套代理（自建清单 / 其它 CDN 的 URL 直接原样返回，
/// 避免把非 GitHub 链接也发给第三方）。直连排在最前，代理兜底在后。
#[cfg_attr(not(feature = "net"), allow(dead_code))]
fn build_candidate_urls(original: &str) -> Vec<String> {
    let mut urls = vec![original.to_string()];
    // 仅对 github.com / objects.githubusercontent.com 的资产启用代理
    let is_github =
        original.contains("github.com") || original.contains("objects.githubusercontent.com");
    if is_github {
        for prefix in ACCELERATE_PREFIXES {
            urls.push(format!("{prefix}{original}"));
        }
    }
    urls
}

/// 把安装包下载到 `dir`，边下边回报进度，落地前做完整性校验。
///
/// 两层健壮性：
/// 1. **断点续传**：先写 `.part`，中断后重试用 `Range` 接着下，服务端不支持
///    （返回 200）则自动从头写；断点越界（416）则清掉临时文件重来。
/// 2. **加速代理兜底**：直连失败或连接被重置时，自动换 GitHub 加速 CDN 源，
///    逐个尝试直到有一个能下。`.part` 已下字节跨源复用。
///
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

    let fail = |reason: String| Error::Source {
        platform: "update".into(),
        reason,
    };

    let candidates = build_candidate_urls(url);
    let mut last_err: Option<String> = None;

    // 逐个候选源尝试；`.part` 已下载的字节跨源保留，换源后从断点继续
    for candidate in &candidates {
        let existing = tokio::fs::metadata(&part)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);

        let mut request = net.client().get(candidate);
        if existing > 0 {
            request = request.header("Range", format!("bytes={existing}-"));
        }
        let response = match request.send().await {
            Ok(resp) => resp,
            Err(err) => {
                // 连接被重置/超时是直连 GitHub 的典型失败，换下一个源
                last_err = Some(format!("{candidate}：{err}"));
                continue;
            }
        };
        let status = response.status();

        // 断点越界：临时文件比远端还大，清掉重来（同一源即可，不必换）
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            let _ = tokio::fs::remove_file(&part).await;
            last_err = Some("断点位置越界，已清理临时文件".to_string());
            continue;
        }
        if !status.is_success() {
            last_err = Some(format!("{candidate}：HTTP {status}"));
            continue;
        }

        let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
        let body_len = response.content_length().unwrap_or(0);
        let total = if resumed {
            existing + body_len
        } else {
            body_len
        };
        // 服务端没给 Content-Length 时用清单里的期望大小兜底
        let total = if total > 0 { total } else { expected_size };
        let mut done = if resumed { existing } else { 0 };

        let mut file = if resumed {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&part)
                .await?
        } else {
            tokio::fs::File::create(&part).await?
        };

        let mut stream = response.bytes_stream();
        // 断流原因先记在局部，等文件句柄收尾后再决定「换源续传」还是「报完整」。
        // 直接在循环里写 `last_err` 的话，后面 `done == 0` 分支一定会把它覆盖掉，
        // 真实的断流原因（比如连接被重置）就丢了。
        let mut interrupted: Option<String> = None;
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(err) => {
                    interrupted = Some(err.to_string());
                    break;
                }
            };
            file.write_all(&chunk).await?;
            done += chunk.len() as u64;
            on_progress(done, total);
        }
        file.flush().await?;
        file.sync_all().await.ok();
        drop(file);

        // 中途断流：已写入的字节留在 `.part` 里，换下一个源续传
        if let Some(err) = interrupted {
            last_err = Some(format!("{candidate}：下载中断（{err}）"));
            continue;
        }

        // 判断这一源是否完整下完；不完整则换下一个源继续
        if done == 0 {
            let _ = tokio::fs::remove_file(&part).await;
            last_err = Some(format!("{candidate}：下载到空内容"));
            continue;
        }
        if total > 0 && done < total {
            // 不删 `.part`，保留断点交给下一个源续传
            last_err = Some(format!("{candidate}：数据不完整（{done}/{total} 字节）"));
            continue;
        }

        // 下完了：落地改名
        if tokio::fs::try_exists(&target).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&target).await;
        }
        tokio::fs::rename(&part, &target).await?;
        return Ok(target);
    }

    let detail = last_err.unwrap_or_else(|| "所有源均不可用".to_string());
    let _ = tokio::fs::remove_file(&part).await;
    Err(fail(format!("安装包下载失败：{detail}")))
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

    #[test]
    fn github_url_gets_acceleration_mirrors() {
        let original = "https://github.com/LunaShiori/melodex/releases/download/v1.0.0/setup.exe";
        let urls = build_candidate_urls(original);
        // 直连排最前
        assert_eq!(urls[0], original);
        // 每个代理前缀各一条，且都拼在原始 URL 后面
        assert_eq!(urls.len(), 1 + ACCELERATE_PREFIXES.len());
        assert_eq!(urls[1], format!("https://mirror.ghproxy.com/{original}"));
        assert!(urls.iter().all(|u| u.contains(original)));
    }

    #[test]
    fn non_github_url_has_no_mirror() {
        let original = "https://cdn.example.com/releases/setup.exe";
        let urls = build_candidate_urls(original);
        assert_eq!(urls, vec![original.to_string()]);
    }
}
