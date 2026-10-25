//! 账号登录。
//!
//! 设计原则：**只读取用户自己账号本来就有的权限**。
//!
//! - 不接收账号密码，只走官方扫码（网易云）或用户自己粘贴的 Cookie；
//! - 不做共享账号池、不绕过付费与版权限制；
//! - 凭据仅保存在本地加密保险库（见 [`crate::vault`]）。

pub mod netease;

use crate::cookie;

/// 扫码登录的一次会话。`qr_url` 交给前端渲染成二维码。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrTicket {
    pub source: String,
    /// 轮询时回传的票据
    pub key: String,
    /// 编码进二维码的内容，手机扫码后跳转官方确认页
    pub qr_url: String,
    /// 票据有效期（秒），前端据此显示倒计时
    pub expires_in: u64,
    /// 已经渲染好的二维码（SVG），前端直接内联即可
    pub qr_svg: String,
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
