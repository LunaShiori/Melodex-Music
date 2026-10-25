//! 下载子系统：命名模板 → 传输 → 标签 → 任务引擎。

#[cfg(feature = "net")]
pub mod engine;
pub mod naming;
pub mod tags;
#[cfg(feature = "net")]
pub mod transfer;

#[cfg(feature = "net")]
pub use engine::{DownloadEngine, EngineEvent, EnqueueInput, Notice, NoticeLevel};
pub use naming::{NamingContext, Resolution};
#[cfg(feature = "net")]
pub use transfer::TransferOutcome;
