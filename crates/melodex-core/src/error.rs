use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::Source;

/// 取不到下载链接的**具体**原因。
///
/// 以前所有失败都折叠成一句「该歌曲没有可用的音频链接」，用户无法判断是自己没登录、
/// 缺会员，还是这首歌本来就没版权——看上去就是「不能用」。这里把原因拆开，
/// 界面才能给出对应的操作按钮（去登录 / 换音源 / 换音质 / 重试）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    /// 音源要求登录态才下发链接。实测 QQ 音乐匿名请求固定返回 `104009 invalidq`。
    NeedLogin,
    /// 需要该音源的付费会员，当前账号没有这项权益。
    VipOnly,
    /// 音源侧没有这个音质档位。实测 QQ 对不存在的档位在 `midurlinfo[].result` 返回 `101404`。
    QualityMissing,
    /// 版权方下架，或当前网络所在地区不可播。
    Copyright,
    /// 网络抖动或音源服务异常，重试可能成功。
    Network,
    /// 拿到了直链，但 CDN 拒绝提供音频（403/404）。多见于链接过期、
    /// 地区的 CDN 节点不可用，或请求缺了必要的头。
    LinkRejected,
    /// 调用参数不合法（前端传了未注册的音源、空 id 等）。
    Invalid,
    /// 兜底：拿到了响应但没读懂原因。
    Unknown,
}

impl Reason {
    /// 界面上跟在歌曲名后面的短语。
    pub fn label(self) -> &'static str {
        match self {
            Reason::NeedLogin => "需要登录",
            Reason::VipOnly => "会员专享",
            Reason::QualityMissing => "无此音质",
            Reason::Copyright => "已下架",
            Reason::Network => "网络异常",
            Reason::LinkRejected => "直链被拒",
            Reason::Invalid => "参数不合法",
            Reason::Unknown => "暂不可用",
        }
    }

    /// 给用户看的完整说明，带下一步该干什么。
    pub fn hint(self) -> &'static str {
        match self {
            Reason::NeedLogin => "该音源必须登录后才提供下载链接，登录后重试即可",
            Reason::VipOnly => "这首歌在该音源属于会员内容，需要开通会员；也可以换一个音源试试",
            Reason::QualityMissing => "该音源没有你要的音质，已自动尝试更低音质，仍不可用",
            Reason::Copyright => "版权方下架或当前地区不可播，换一个音源可能还有",
            Reason::Network => "网络或音源服务暂时异常，稍后重试",
            Reason::LinkRejected => {
                "音源给的播放地址被它自己的 CDN 拒绝了（常见于链接过期或该地区节点不可用），重试或换音源"
            }
            Reason::Invalid => "调用参数有问题，换一首歌或重启应用",
            Reason::Unknown => "音源没有返回可用链接，稍后重试或换一个音源",
        }
    }

    /// 是否值得再点一次「重试」。
    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Reason::Network | Reason::LinkRejected | Reason::Unknown
        )
    }

    /// 是否可以通过「去登录」解决。
    pub fn is_login_fixable(self) -> bool {
        matches!(self, Reason::NeedLogin | Reason::VipOnly)
    }

    /// 多个音质档位都失败时，保留信息量最大的那个原因：
    /// 「要登录」比「没这个音质」对用户有用得多。
    pub fn rank(self) -> u8 {
        match self {
            Reason::NeedLogin => 5,
            Reason::VipOnly => 4,
            Reason::Copyright => 3,
            Reason::QualityMissing => 2,
            Reason::LinkRejected => 1,
            Reason::Network => 1,
            Reason::Invalid => 0,
            Reason::Unknown => 0,
        }
    }
}

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

    /// 拿到了服务端响应，但里面有明确的原因是「这首不能下」。
    /// `message` 已经是给用户看的完整中文句子，界面可直接展示。
    #[error("{message}")]
    Unavailable {
        /// 出错的是哪个音源。字段名不能叫 `source`，否则 thiserror 会把它当成
        /// 错误链里的「原因」并强制要求实现 `std::error::Error`。
        origin: Source,
        reason: Reason,
        message: String,
    },

    /// 直链已经拿到，但 CDN 拒绝提供内容（403/404 居多）。
    /// 下载层抛出，引擎据此决定「重取一次链接」还是按「直链被拒」上报。
    #[error("下载被拒绝：HTTP {status}")]
    HttpStatus { status: u16 },

    /// 没有任何原因信息的兜底失败（旧路径仍在用）。
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

    /// 构造一条带结构化原因的「这首下不了」。
    pub fn unavailable(source: Source, reason: Reason) -> Self {
        Error::Unavailable {
            origin: source,
            reason,
            message: format!("{}：{}", source.label(), reason.hint()),
        }
    }

    /// 同上，但用音源自己返回的原文作为补充说明。
    pub fn unavailable_with(source: Source, reason: Reason, detail: impl AsRef<str>) -> Self {
        let detail = detail.as_ref().trim();
        let message = if detail.is_empty() {
            format!("{}：{}", source.label(), reason.hint())
        } else {
            format!("{}：{}（{detail}）", source.label(), reason.hint())
        };
        Error::Unavailable {
            origin: source,
            reason,
            message,
        }
    }

    /// 出错的是哪个音源。界面按音源显示「去登录」入口时用得上。
    pub fn origin(&self) -> Option<Source> {
        match self {
            Error::Unavailable { origin, .. } => Some(*origin),
            _ => None,
        }
    }

    /// 取出结构化原因，非「下不了」类错误返回 `None`。
    pub fn reason(&self) -> Option<Reason> {
        match self {
            Error::Unavailable { reason, .. } => Some(*reason),
            Error::NoPlayableUrl => Some(Reason::Unknown),
            Error::HttpStatus { status } => {
                if *status == 403 || *status == 404 {
                    Some(Reason::LinkRejected)
                } else {
                    Some(Reason::Network)
                }
            }
            #[cfg(feature = "net")]
            Error::Http(_) => Some(Reason::Network),
            _ => None,
        }
    }

    /// 是否属于「换一个音质/音源还能再试一次」的软失败。
    #[cfg(feature = "net")]
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Http(_) | Error::NoPlayableUrl | Error::HttpStatus { .. } => true,
            Error::Unavailable { reason, .. } => reason.is_retryable(),
            _ => false,
        }
    }

    #[cfg(not(feature = "net"))]
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::NoPlayableUrl => true,
            Error::Unavailable { reason, .. } => reason.is_retryable(),
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// 多个音源 / 多个音质档位都失败时，保留「对用户最有用」的那条原因。
///
/// 「该音源必须登录后才提供下载链接」比「该音源没有你要的音质」更能指导下一步动作，
/// 所以按 [`Reason::rank`] 取最高的，而不是简单取最后一条。
pub fn keep_most_actionable(best: &mut Option<Error>, candidate: Error) {
    let rank = candidate.reason().map(Reason::rank).unwrap_or(0);
    let current = best
        .as_ref()
        .and_then(Error::reason)
        .map(Reason::rank)
        .unwrap_or(0);
    if best.is_none() || rank > current {
        *best = Some(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_is_not_retryable() {
        assert!(!Error::decode("x").is_retryable());
        assert!(Error::NoPlayableUrl.is_retryable());
    }

    #[test]
    fn need_login_beats_quality_missing() {
        assert!(Reason::NeedLogin.rank() > Reason::QualityMissing.rank());
        assert!(Reason::VipOnly.rank() > Reason::QualityMissing.rank());
        assert!(Reason::Copyright.rank() > Reason::Network.rank());
    }

    #[test]
    fn only_network_and_unknown_are_retryable() {
        assert!(Reason::Network.is_retryable());
        assert!(Reason::Unknown.is_retryable());
        assert!(!Reason::NeedLogin.is_retryable());
        assert!(!Reason::VipOnly.is_retryable());
        assert!(!Reason::QualityMissing.is_retryable());
    }

    #[test]
    fn login_fixable_covers_need_login_and_vip() {
        assert!(Reason::NeedLogin.is_login_fixable());
        assert!(Reason::VipOnly.is_login_fixable());
        assert!(!Reason::Copyright.is_login_fixable());
    }

    #[test]
    fn http_403_means_link_rejected_and_retryable() {
        let denied = Error::HttpStatus { status: 403 };
        assert_eq!(denied.reason(), Some(Reason::LinkRejected));
        assert!(denied.is_retryable());

        let missing = Error::HttpStatus { status: 404 };
        assert_eq!(missing.reason(), Some(Reason::LinkRejected));

        let broken = Error::HttpStatus { status: 500 };
        assert_eq!(broken.reason(), Some(Reason::Network));
    }

    #[test]
    fn message_carries_source_and_hint() {
        let err = Error::unavailable(Source::Qq, Reason::NeedLogin);
        let text = err.to_string();
        assert!(text.contains("QQ 音乐"), "{text}");
        assert!(text.contains("登录"), "{text}");
        assert_eq!(err.reason(), Some(Reason::NeedLogin));
        assert_eq!(err.origin(), Some(Source::Qq));
    }

    #[test]
    fn detail_is_appended_in_parentheses() {
        let err = Error::unavailable_with(Source::Qq, Reason::NeedLogin, "104009 invalidq");
        let text = err.to_string();
        assert!(text.contains("104009 invalidq"), "{text}");
    }

    #[test]
    fn reason_serializes_camel_case() {
        assert_eq!(
            serde_json::to_value(Reason::NeedLogin).unwrap(),
            serde_json::json!("needLogin")
        );
        assert_eq!(
            serde_json::to_value(Reason::QualityMissing).unwrap(),
            serde_json::json!("qualityMissing")
        );
    }
}
