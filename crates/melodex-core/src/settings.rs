use serde::{Deserialize, Serialize};

use crate::model::Source;

/// 重复文件的处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicatePolicy {
    /// 跳过，不下载
    Skip,
    /// 覆盖已有文件
    Overwrite,
    /// 保留两份，文件名加序号
    KeepBoth,
}

impl Default for DuplicatePolicy {
    fn default() -> Self {
        DuplicatePolicy::KeepBoth
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::System
    }
}

/// 全部可配置项。前端设置页与该结构一一对应，改动即持久化。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 下载目录，空字符串表示使用平台默认目录
    pub download_dir: String,
    /// 目标音质（kbps）
    pub bitrate: u32,
    /// 目标音质不可用时的降级链
    pub fallback_bitrates: Vec<u32>,
    /// 优先从哪个音源取链接
    pub prefer_source: Option<Source>,
    /// 文件命名模板，支持 {title} {artist} {album} {source} {bitrate} {index}
    pub naming_template: String,
    /// 多歌手之间的连接符
    pub artist_separator: String,
    /// 同时下载的任务数
    pub concurrency: usize,
    /// 是否写入 ID3 标签
    pub write_tags: bool,
    /// 是否内嵌封面
    pub embed_cover: bool,
    /// 是否额外保存 .lrc 歌词
    pub save_lyric: bool,
    /// 重复文件策略
    pub duplicate_policy: DuplicatePolicy,
    /// 界面主题
    pub theme: ThemeMode,
    /// 启用的音源
    pub sources: Vec<Source>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: String::new(),
            bitrate: 320,
            fallback_bitrates: vec![320, 192, 128],
            prefer_source: None,
            naming_template: "{artist} - {title}".into(),
            artist_separator: " / ".into(),
            concurrency: 3,
            write_tags: true,
            embed_cover: true,
            save_lyric: true,
            duplicate_policy: DuplicatePolicy::KeepBoth,
            theme: ThemeMode::System,
            sources: Source::ALL.to_vec(),
        }
    }
}

impl Settings {
    /// 收敛非法输入，防止用户把并发调成 999 或填入未支持音质。
    pub fn sanitized(mut self) -> Self {
        self.concurrency = self.concurrency.clamp(1, 8);
        if self.bitrate == 0 {
            self.bitrate = 320;
        }
        self.fallback_bitrates.retain(|b| *b > 0);
        if self.fallback_bitrates.is_empty() {
            self.fallback_bitrates = vec![self.bitrate, 192, 128];
        }
        if self.naming_template.trim().is_empty() {
            self.naming_template = "{artist} - {title}".into();
        }
        if self.artist_separator.is_empty() {
            self.artist_separator = " / ".into();
        }
        if self.sources.is_empty() {
            self.sources = Source::ALL.to_vec();
        }
        self.sources.dedup();
        self
    }

    /// 音质降级链：目标音质优先，其后按配置依次尝试。
    pub fn bitrate_chain(&self) -> Vec<u32> {
        let mut chain = vec![self.bitrate];
        for value in &self.fallback_bitrates {
            if !chain.contains(value) {
                chain.push(*value);
            }
        }
        chain
    }
}

/// 平台默认下载目录。
pub fn default_download_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            return std::path::PathBuf::from(profile).join("Music").join("Melodex");
        }
    }
    #[cfg(target_os = "android")]
    {
        // Android 由外壳层注入实际目录，这里只作兜底
        return std::path::PathBuf::from("/storage/emulated/0/Download/Melodex");
    }
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join("Music").join("Melodex");
        }
    }
    std::path::PathBuf::from("Melodex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let settings = Settings::default();
        assert_eq!(settings.bitrate, 320);
        assert!(settings.concurrency >= 1);
        assert!(settings.sources.contains(&Source::Netease));
    }

    #[test]
    fn sanitize_clamps_concurrency() {
        let mut settings = Settings::default();
        settings.concurrency = 999;
        assert_eq!(settings.clone().sanitized().concurrency, 8);
        settings.concurrency = 0;
        assert_eq!(settings.sanitized().concurrency, 1);
    }

    #[test]
    fn bitrate_chain_dedups() {
        let settings = Settings {
            bitrate: 320,
            fallback_bitrates: vec![320, 128, 128],
            ..Default::default()
        };
        assert_eq!(settings.bitrate_chain(), vec![320, 128]);
    }

    #[test]
    fn deserializes_partial_json() {
        let settings: Settings = serde_json::from_str(r#"{"bitrate":128}"#).unwrap();
        assert_eq!(settings.bitrate, 128);
        assert_eq!(settings.concurrency, 3);
    }
}
