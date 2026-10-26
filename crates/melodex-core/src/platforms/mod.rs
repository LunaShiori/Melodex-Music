//! 音源适配器集合。
//!
//! 每个子模块是一个独立的音源实现，只依赖 [`crate::platform::MusicPlatform`]
//! 契约与 [`crate::net::Net`]，彼此之间没有耦合。

pub mod kuwo;
pub mod netease;
pub mod qq;
