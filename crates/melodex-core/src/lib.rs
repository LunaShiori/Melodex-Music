//! Melodex 共享核心。
//!
//! 分层：`platforms`（音源适配器）→ `platform`（契约与注册表）→ `aggregator`
//! （跨源聚合）→ `download`（任务与下载）。上层外壳（Tauri 桌面端 / Android）
//! 只依赖本 crate 的公开 API，不感知任何音源细节。
//!
//! 纯逻辑模块（模型、歌词、命名、标签、任务、配置）不依赖任何网络库，
//! 可通过 `--no-default-features` 单独编译与测试。

#[cfg(feature = "net")]
pub mod aggregator;
pub mod cookie;
pub mod download;
pub mod error;
#[cfg(feature = "net")]
pub mod login;
pub mod lyric;
pub mod model;
#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "net")]
pub mod platform;
#[cfg(feature = "net")]
pub mod platforms;
pub mod pseudo_json;
pub mod qr;
pub mod settings;
pub mod task;
pub mod update;
pub mod vault;

#[cfg(feature = "net")]
pub use aggregator::Aggregator;
#[cfg(feature = "net")]
pub use download::engine::{DownloadEngine, EngineEvent, EnqueueInput, Notice, NoticeLevel};
pub use error::{Error, Result};
pub use model::{
    AggregatedPage, Fee, Lyric, PlayUrl, PlaylistDetail, SearchPage, Source, SourceReport, Track,
    Variant,
};
#[cfg(feature = "net")]
pub use platform::{CredStore, MusicPlatform, Registry, SearchQuery};
pub use settings::{default_download_dir, DuplicatePolicy, Settings, ThemeMode};
pub use update::{ReleaseAsset, UpdateInfo};

/// 当前核心版本号，随包发布。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 统一 UA，抓取图片/音频时也用同一个。
#[cfg(feature = "net")]
pub const USER_AGENT: &str = net::UA;

/// 便捷入口：构造一个可用的聚合器。
#[cfg(feature = "net")]
pub fn build_aggregator(creds: CredStore) -> Result<Aggregator> {
    let net = net::Net::new()?;
    Ok(Aggregator::new(Registry::new(net, creds)))
}
