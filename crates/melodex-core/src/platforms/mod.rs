//! 音源适配器集合。
//!
//! 每个子模块是一个独立的音源实现，只依赖 [`crate::platform::MusicPlatform`]
//! 契约与 [`crate::net::Net`]，彼此之间没有耦合。

pub mod kuwo;
pub mod netease;
pub mod qq;

/// 把网易云的档位名翻回标称码率，仅在服务端没回 `br` 时兜底。
pub fn guess_bitrate(level: &str) -> u32 {
    match level {
        "hires" => 999,
        "lossless" => 999,
        "exhigh" => 320,
        "higher" => 192,
        _ => 128,
    }
}
