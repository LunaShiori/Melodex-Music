use thiserror::Error;

/// 核心层的统一错误类型。所有音源适配器都把各自的失败折叠到这里，
/// 聚合层才能对单个音源的失败做降级而不影响整体结果。
#[derive(Debug, Error)]
pub enum Error {
    #[cfg(feature = "net")]
    #[error("网络请求失败：{0}")]
    Http(#[from] reqwest::Error),

    #[error("响应解析失败：{0}")]
    Decode(String),

    #[error("音源 {platform} 不可用：{reason}")]
    Source { platform: String, reason: String },

    #[error("该歌曲没有可用的音频链接")]
    NoPlayableUrl,

    #[error("文件操作失败：{0}")]
    Io(#[from] std::io::Error),

    #[error("任务不存在：{0}")]
    TaskNotFound(String),

    #[error("参数不合法：{0}")]
    Invalid(String),
}

impl Error {
    pub fn decode(msg: impl Into<String>) -> Self {
        Error::Decode(msg.into())
    }

    /// 是否属于「换一个音质/音源还能再试一次」的软失败。
    #[cfg(feature = "net")]
    pub fn is_retryable(&self) -> bool {
        matches!(self, Error::Http(_) | Error::NoPlayableUrl)
    }

    #[cfg(not(feature = "net"))]
    pub fn is_retryable(&self) -> bool {
        matches!(self, Error::NoPlayableUrl)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_is_not_retryable() {
        assert!(!Error::decode("x").is_retryable());
        assert!(Error::NoPlayableUrl.is_retryable());
    }
}
