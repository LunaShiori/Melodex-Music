use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::Source;
use crate::quality::QualityLevel;

/// 重复文件的处理策略。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicatePolicy {
    /// 跳过，不下载
    Skip,
    /// 覆盖已有文件
    Overwrite,
    /// 保留两份，文件名加序号
    #[default]
    KeepBoth,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

/// 界面强调色（主题色）。与明暗主题正交，用户可以自由组合。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    /// 紫罗兰（默认）
    #[default]
    Violet,
    /// 靛蓝
    Indigo,
    /// 青碧
    Teal,
    /// 玫红
    Rose,
    /// 琥珀
    Amber,
    /// 翠绿
    Green,
}

/// 全部可配置项。前端设置页与该结构一一对应，改动即持久化。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 下载目录，空字符串表示使用平台默认目录
    pub download_dir: String,
    /// 全局默认目标音质。
    ///
    /// 存的是跨平台对齐的「等级」，具体落到哪个档位由各平台的档位表决定
    /// （QQ 的 lossless 是 SQ、网易的 lossless 是无损音质、酷我没有这一档就落到它的上限）。
    pub quality: QualityLevel,
    /// 分平台覆盖：某个音源单独指定目标音质，缺省时用 `quality`
    pub quality_overrides: BTreeMap<Source, QualityLevel>,
    /// 目标档位取不到时是否自动降级（关掉就只试选中的那一档）
    pub auto_fallback: bool,
    /// 配置结构版本，用于一次性迁移；当前版本固定写 2
    #[serde(default)]
    pub schema: u32,
    /// 旧版的全局音质（kbps），仅用于迁移，不再写回文件
    #[serde(default, skip_serializing)]
    pub bitrate: u32,
    /// 旧版的降级链（kbps），仅用于迁移，不再写回文件
    #[serde(default, skip_serializing)]
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
    /// 界面强调色（主题色）
    pub accent: Accent,
    /// 启用的音源
    pub sources: Vec<Source>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: String::new(),
            quality: QualityLevel::Lossless,
            quality_overrides: BTreeMap::new(),
            auto_fallback: true,
            schema: CONFIG_SCHEMA,
            bitrate: 0,
            fallback_bitrates: Vec::new(),
            prefer_source: None,
            naming_template: "{artist} - {title}".into(),
            artist_separator: " / ".into(),
            concurrency: 3,
            write_tags: true,
            embed_cover: true,
            save_lyric: true,
            duplicate_policy: DuplicatePolicy::KeepBoth,
            theme: ThemeMode::System,
            accent: Accent::Violet,
            sources: Source::ALL.to_vec(),
        }
    }
}

/// 配置结构版本。每次做一次性的字段迁移就 +1，迁移代码只对旧版本生效。
pub const CONFIG_SCHEMA: u32 = 2;

impl Settings {
    /// 收敛非法输入，并执行一次性迁移。
    pub fn sanitized(mut self) -> Self {
        self.concurrency = self.concurrency.clamp(1, 8);
        if self.schema < CONFIG_SCHEMA {
            // v1 只有裸 bitrate，用户当时选的 999 其实是「能拿多高拿多高」。
            // 注意不能用 `quality == Standard` 判断是否「用户没设过」——容器级
            // `#[serde(default)]` 会先把 quality 填成默认值，只能看 bitrate 自己。
            if self.bitrate > 0 {
                self.quality = QualityLevel::from_bitrate(self.bitrate);
            }
            self.schema = CONFIG_SCHEMA;
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
        // 覆盖项里不允许出现没启用的音源，否则界面上会出现选不中的行
        self.quality_overrides
            .retain(|source, _| self.sources.contains(source));
        self
    }

    /// 某个音源的目标音质：优先取分平台设置，其次全局默认。
    pub fn quality_of(&self, source: Source) -> QualityLevel {
        self.quality_overrides
            .get(&source)
            .copied()
            .unwrap_or(self.quality)
    }

    /// 旧接口：按裸码率给出目标。供只需要展示的场景使用。
    pub fn quality_bitrate(&self, source: Source) -> u32 {
        crate::quality::find(source, self.quality_of(source))
            .map(|item| item.bitrate)
            .unwrap_or(128)
    }
}

/// 平台默认下载目录。
///
/// 每个目标平台只编译出**一个** `let dir`，避免用 `return` 早退导致其余分支成为
/// 不可达代码（Android 目标上曾因此触发 `unreachable_code` 警告）。
pub fn default_download_dir() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    let dir = std::path::PathBuf::from("/storage/emulated/0/Download/Melodex");

    #[cfg(target_os = "windows")]
    let dir = std::env::var("USERPROFILE")
        .map(|profile| {
            std::path::PathBuf::from(profile)
                .join("Music")
                .join("Melodex")
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("Melodex"));

    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    let dir = std::env::var("HOME")
        .map(|home| std::path::PathBuf::from(home).join("Music").join("Melodex"))
        .unwrap_or_else(|_| std::path::PathBuf::from("Melodex"));

    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let settings = Settings::default();
        assert_eq!(settings.quality, QualityLevel::Lossless);
        assert!(settings.auto_fallback);
        assert!(settings.concurrency >= 1);
        assert!(settings.sources.contains(&Source::Netease));
    }

    #[test]
    fn sanitize_clamps_concurrency() {
        let too_many = Settings {
            concurrency: 999,
            ..Default::default()
        };
        assert_eq!(too_many.sanitized().concurrency, 8);

        let too_few = Settings {
            concurrency: 0,
            ..Default::default()
        };
        assert_eq!(too_few.sanitized().concurrency, 1);
    }

    #[test]
    fn per_source_quality_wins_over_global() {
        let mut settings = Settings {
            quality: QualityLevel::Lossless,
            ..Default::default()
        };
        settings
            .quality_overrides
            .insert(Source::Kuwo, QualityLevel::Standard);
        assert_eq!(settings.quality_of(Source::Netease), QualityLevel::Lossless);
        assert_eq!(settings.quality_of(Source::Kuwo), QualityLevel::Standard);
        // 档位落到平台表上：酷我没有无损，展示码率应取它唯一那档
        assert_eq!(settings.quality_bitrate(Source::Kuwo), 128);
        assert_eq!(settings.quality_bitrate(Source::Netease), 1000);
    }

    #[test]
    fn drops_overrides_for_disabled_sources() {
        let mut settings = Settings {
            sources: vec![Source::Qq],
            ..Default::default()
        };
        settings
            .quality_overrides
            .insert(Source::Netease, QualityLevel::Master);
        let cleaned = settings.sanitized();
        assert!(cleaned.quality_overrides.is_empty());
    }

    #[test]
    fn migrates_legacy_bitrate_once() {
        // v1 配置：没有 schema，只有裸 bitrate
        let legacy: Settings = serde_json::from_str(r#"{"bitrate":999}"#).unwrap();
        let migrated = legacy.sanitized();
        assert_eq!(migrated.quality, QualityLevel::Lossless);
        assert_eq!(migrated.schema, CONFIG_SCHEMA);

        // 迁移过的配置再次 sanitize 不应被旧字段覆盖（用户可能故意改回标准）
        let mut again = migrated;
        again.quality = QualityLevel::Standard;
        assert_eq!(again.sanitized().quality, QualityLevel::Standard);
    }

    #[test]
    fn deserializes_partial_json() {
        let settings: Settings = serde_json::from_str(r#"{"concurrency":2}"#).unwrap();
        assert_eq!(settings.concurrency, 2);
        assert_eq!(settings.quality, QualityLevel::Lossless);
    }

    #[test]
    fn legacy_fields_are_not_written_back() {
        let settings = Settings {
            bitrate: 320,
            fallback_bitrates: vec![320, 128],
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(!json.contains("fallbackBitrates"));
        assert!(json.contains("\"schema\":2"));
    }

    #[test]
    fn accent_serializes_lowercase_and_defaults_to_violet() {
        let settings: Settings = serde_json::from_str(r#"{"accent":"teal"}"#).unwrap();
        assert_eq!(settings.accent, Accent::Teal);
        assert_eq!(Settings::default().accent, Accent::Violet);
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(json.contains("\"accent\":\"violet\""), "{json}");
    }
}
