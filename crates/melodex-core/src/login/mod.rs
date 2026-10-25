//! 账号登录。
//!
//! 设计原则：**只读取用户自己账号本来就有的权限**。
//!
//! - 不接收账号密码，只走官方扫码（网易云）或用户自己粘贴的 Cookie；
//! - 不做共享账号池、不绕过付费与版权限制；
//! - 凭据仅保存在本地加密保险库（见 [`crate::vault`]）。

pub mod mqtt;
pub mod netease;
pub mod qq;

use crate::cookie;

/// 扫码登录的一次会话。
///
/// 两种音源给的物料不一样：网易云给的是一个 URL，我们本地渲染成 SVG；
/// QQ 互联只给一张 PNG 位图，没有可编码的内容，因此这里同时保留两种形态，
/// 前端按哪个有值决定用 `<svg>` 还是 `<img>`。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrTicket {
    pub source: String,
    /// 轮询时回传的票据
    pub key: String,
    /// 编码进二维码的内容；QQ 的二维码是位图，这里为空串
    pub qr_url: String,
    /// 票据有效期（秒），前端据此显示倒计时
    pub expires_in: u64,
    /// 本地渲染好的二维码（SVG），前端直接内联
    pub qr_svg: Option<String>,
    /// 音源直接下发的二维码位图，形如 `data:image/png;base64,...`
    pub qr_image: Option<String>,
}

/// 轮询状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QrStatus {
    /// 还没扫
    Waiting,
    /// 已扫码，等待手机上点确认
    Scanned,
    /// 确认成功，`cookie` 已就绪
    Confirmed,
    /// 二维码过期，需要重新生成
    Expired,
    /// 登录链路本身出错（网络、协议、换凭据失败），`message` 里有具体原因
    Error,
}

/// 一次轮询的结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrPoll {
    pub status: QrStatus,
    /// 仅在 `Confirmed` 时有值
    pub cookie: Option<String>,
    /// 仅在 `Confirmed` 时尽力获取
    pub nickname: Option<String>,
    /// 直接展示给用户的文案（音源原样返回的 message）
    pub message: Option<String>,
}

impl QrPoll {
    pub(crate) fn state(status: QrStatus, message: Option<String>) -> Self {
        Self {
            status,
            cookie: None,
            nickname: None,
            message,
        }
    }
}

/// 从 `Set-Cookie` 响应头拼出 Cookie 串。
pub fn cookies_from_headers(headers: &reqwest::header::HeaderMap) -> String {
    let values = headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok());
    cookie::from_set_cookie(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_multiple_set_cookie_headers() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append(
            reqwest::header::SET_COOKIE,
            "MUSIC_U=token123; Path=/; HttpOnly".parse().unwrap(),
        );
        headers.append(
            reqwest::header::SET_COOKIE,
            "__csrf=csrf456; Path=/".parse().unwrap(),
        );
        assert_eq!(
            cookies_from_headers(&headers),
            "MUSIC_U=token123; __csrf=csrf456"
        );
    }
}
