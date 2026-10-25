//! 网易云音乐扫码登录。
//!
//! 流程与官方客户端一致，全程不接触账号密码：
//!
//! 1. `GET /api/login/qrcode/unikey?type=1` 拿 `unikey`；
//! 2. 前端把 `https://music.163.com/login?codekey=<unikey>` 渲染成二维码；
//! 3. 手机扫码确认后，轮询 `/api/login/qrcode/client/login` 返回 803，
//!    响应头里的 `Set-Cookie` 带着 `MUSIC_U`，那就是登录态。

use serde_json::Value;

use crate::cookie;
use crate::error::{Error, Result};
use crate::login::{cookies_from_headers, QrPoll, QrStatus, QrTicket};
use crate::net::Net;

const BASE: &str = "https://music.163.com";
const REFERER: &str = "https://music.163.com/";
/// 申请二维码票据时必须声明 PC 端，否则拿不到可用的 unikey
const ANON_COOKIE: &str = "os=pc; appver=8.9.70";
/// 官方二维码有效期约 3 分钟
const QR_TTL_SECS: u64 = 180;

fn headers(cookie: &str) -> [(&str, &str); 2] {
    [("Referer", REFERER), ("Cookie", cookie)]
}

/// 申请一个新的二维码票据。
pub async fn start(net: &Net) -> Result<QrTicket> {
    let url = format!("{BASE}/api/login/qrcode/unikey?type=1");
    let body = net.text(&url, &headers(ANON_COOKIE)).await?;
    let value: Value = serde_json::from_str(&body)
        .map_err(|err| Error::decode(format!("二维码票据解析失败：{err}")))?;
    let key = value
        .get("unikey")
        .and_then(Value::as_str)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| Error::Source {
            platform: "netease".into(),
            reason: "网易云没有返回二维码票据，请稍后重试".into(),
        })?;

    let qr_url = format!("https://music.163.com/login?codekey={key}");
    let qr_svg = crate::qr::render_svg(&qr_url).ok_or_else(|| Error::Source {
        platform: "netease".into(),
        reason: "二维码渲染失败".into(),
    })?;

    Ok(QrTicket {
        source: "netease".into(),
        key: key.to_string(),
        qr_url,
        expires_in: QR_TTL_SECS,
        qr_svg: Some(qr_svg),
        qr_image: None,
    })
}

/// 轮询扫码结果。
pub async fn poll(net: &Net, key: &str) -> Result<QrPoll> {
    if key.trim().is_empty() {
        return Err(Error::Invalid("缺少二维码票据".into()));
    }
    let url = format!("{BASE}/api/login/qrcode/client/login?key={key}&type=1");
    let response = net.get(&url, &headers(ANON_COOKIE)).await?;
    let from_header = cookies_from_headers(response.headers());
    let body = response.text().await?;
    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string);

    match code {
        // 等待扫码
        801 => Ok(QrPoll::state(QrStatus::Waiting, message)),
        // 已扫码，等待手机上确认
        802 => Ok(QrPoll::state(QrStatus::Scanned, message)),
        // 二维码过期
        800 => Ok(QrPoll::state(QrStatus::Expired, message)),
        // 登录成功
        803 => {
            // 少数情况下 Cookie 在响应体的 `cookie` 字段里，两种都兼容
            let body_cookie = value
                .get("cookie")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let merged = cookie::merge(&from_header, body_cookie);
            if cookie::value_of(&merged, "MUSIC_U").is_none() {
                return Err(Error::Source {
                    platform: "netease".into(),
                    reason: "登录响应里没有 MUSIC_U，请重新扫码".into(),
                });
            }
            let nickname = profile(net, &merged).await;
            Ok(QrPoll {
                status: QrStatus::Confirmed,
                cookie: Some(merged),
                nickname,
                message,
            })
        }
        502 | 504 => Err(Error::Source {
            platform: "netease".into(),
            reason: message.unwrap_or_else(|| "网易云服务暂时不可用".into()),
        }),
        other => Err(Error::Source {
            platform: "netease".into(),
            reason: format!("登录未完成（code={other}）{}", message.unwrap_or_default()),
        }),
    }
}

/// 用 Cookie 查一次账号信息，拿昵称做登录态展示。失败不影响登录本身。
pub async fn profile(net: &Net, cookie: &str) -> Option<String> {
    let body = net
        .text(&format!("{BASE}/api/nuser/account/get"), &headers(cookie))
        .await
        .ok()?;
    let value: Value = serde_json::from_str(&body).ok()?;
    value
        .pointer("/profile/nickname")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只有 803 才认作成功；其余状态码返回结构化状态而不是错误。
    #[test]
    fn status_codes_are_typed() {
        assert_eq!(
            serde_json::to_value(QrStatus::Waiting).unwrap(),
            serde_json::json!("waiting")
        );
        assert_eq!(
            serde_json::to_value(QrStatus::Confirmed).unwrap(),
            serde_json::json!("confirmed")
        );
    }

    #[test]
    fn ticket_serializes_camel_case() {
        let ticket = QrTicket {
            source: "netease".into(),
            key: "abc".into(),
            qr_url: "https://music.163.com/login?codekey=abc".into(),
            expires_in: 180,
            qr_svg: crate::qr::render_svg("https://music.163.com/login?codekey=abc"),
            qr_image: None,
        };
        let value = serde_json::to_value(ticket).unwrap();
        assert_eq!(value["qrUrl"], "https://music.163.com/login?codekey=abc");
        assert_eq!(value["expiresIn"], 180);
        assert!(value["qrSvg"].as_str().is_some());
        assert!(value["qrImage"].is_null());
    }
}
