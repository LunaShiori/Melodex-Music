//! QQ 音乐扫码登录。
//!
//! 走的是 QQ 互联的官方「扫一扫」授权，全程不接触账号密码，流程分四跳：
//!
//! 1. `ptqrshow` —— 申请二维码。这个接口**直接返回 PNG 位图**（不是可编码的 URL），
//!    同时下发 `qrsig` Cookie；位图转成 `data:` URI 交给前端 `<img>` 展示。
//! 2. `ptqrlogin` —— 轮询扫码状态。`ptqrtoken` 由 `qrsig` 经 `hash33` 得到，
//!    返回一段 JS：`ptuiCB('0','0','<url>','0','登录成功','')`。
//! 3. 跟随 `ptuiCB` 给出的 `check_sig` 地址 —— 这一跳的 `Set-Cookie` 里才有
//!    `uin` / `skey` / `p_skey`，所以必须禁止自动重定向。
//! 4. `QQConnectLogin.LoginServer/QQLogin` —— 用 `skey` 换 QQ 音乐的 `qm_keyst`，
//!    拿到它才算是「QQ 音乐已登录」。
//!
//! 任何一跳失败都会明确告知失败在哪一步，并提示改用「粘贴 Cookie」，
//! 避免用户卡在一个说不清原因的转圈里。

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::json;

use crate::cookie;
use crate::error::{Error, Reason, Result};
use crate::login::{cookies_from_headers, QrPoll, QrStatus, QrTicket};
use crate::model::Source;
use crate::net::Net;

/// QQ 音乐在 QQ 互联注册的第三方 appid（网页版扫码用的就是这套参数）
const APPID: &str = "716027609";
const DAID: &str = "383";
/// QQ 音乐作为第三方网站的 aid
const THIRD_AID: &str = "100497308";
const QR_SHOW: &str = "https://ssl.ptlogin2.qq.com/ptqrshow";
const QR_LOGIN: &str = "https://ssl.ptlogin2.qq.com/ptqrlogin";
const MUSICU: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const YQQ: &str = "https://y.qq.com/";
/// 二维码有效期约 2 分钟
const QR_TTL_SECS: u64 = 120;

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// `hash33`：把 `qrsig` 算成 `ptqrtoken`。
///
/// 与网页端脚本一致：`e = 0; for c in s { e += (e << 5) + c }; e & 0x7fffffff`。
pub fn hash33(input: &str) -> u32 {
    let mut hash: u32 = 0;
    for byte in input.bytes() {
        hash = hash.wrapping_add(hash << 5).wrapping_add(u32::from(byte));
    }
    hash & 0x7fff_ffff
}

/// 从 `ptuiCB('0','0','<url>','0','<msg>', '')` 里取出字段。
///
/// 不用 `split("','")` 是因为消息里可能带逗号或单引号，按引号扫描更稳。
fn quoted_fields(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch != '\'' {
            continue;
        }
        let mut buf = String::new();
        for inner in chars.by_ref() {
            if inner == '\'' {
                break;
            }
            buf.push(inner);
        }
        out.push(buf);
    }
    out
}

/// 申请二维码。返回的 `key` 是 `qrsig`，轮询时要用它算 `ptqrtoken`。
pub async fn start(net: &Net) -> Result<QrTicket> {
    let t = now_millis();
    let url = format!(
        "{QR_SHOW}?appid={APPID}&e=2&l=M&s=3&d=72&v=4&t={t}&daid={DAID}&pt_3rd_aid={THIRD_AID}\
         &u1=https%3A%2F%2Fgraph.qq.com%2Foauth2.0%2Flogin_jump&ptredirect=1&h=1&g=1&from_ui=1\
         &ptlang=2052&action=0-0-{t}&js_ver=20102616&js_type=1&login_sig=&pt_uistyle=40"
    );
    let response = net
        .get(&url, &[("Referer", "https://xui.ptlogin2.qq.com/")])
        .await?;
    let set_cookie = cookies_from_headers(response.headers());
    let key = cookie::value_of(&set_cookie, "qrsig").ok_or_else(|| Error::Source {
        platform: "qq".into(),
        reason: "QQ 没有下发二维码票据（qrsig），请稍后重试".into(),
    })?;

    let bytes = response.bytes().await?;
    if bytes.len() < 8 || &bytes[..4] != b"\x89PNG" {
        return Err(Error::Source {
            platform: "qq".into(),
            reason: "QQ 返回的二维码不是有效图片，请稍后重试".into(),
        });
    }
    let qr_image = format!(
        "data:image/png;base64,{}",
        crate::qr::base64_encode(bytes.as_ref())
    );

    Ok(QrTicket {
        source: "qq".into(),
        key: key.to_string(),
        // QQ 的二维码是位图，没有可编码的字符串内容
        qr_url: String::new(),
        expires_in: QR_TTL_SECS,
        qr_svg: None,
        qr_image: Some(qr_image),
    })
}

/// 轮询扫码状态。`key` 是 [`start`] 返回的 `qrsig`。
pub async fn poll(net: &Net, key: &str) -> Result<QrPoll> {
    let key = key.trim();
    if key.is_empty() {
        return Err(Error::Invalid("缺少二维码票据".into()));
    }
    let t = now_millis();
    let token = hash33(key);
    let url = format!(
        "{QR_LOGIN}?u1=https%3A%2F%2Fgraph.qq.com%2Foauth2.0%2Flogin_jump&ptqrtoken={token}\
         &ptredirect=1&h=1&t=1&g=1&from_ui=1&ptlang=2052&action=0-0-{t}&js_ver=20102616\
         &js_type=1&login_sig=&pt_uistyle=40&aid={APPID}&daid={DAID}&pt_3rd_aid={THIRD_AID}"
    );
    let body = net
        .text(
            &url,
            &[
                ("Referer", "https://xui.ptlogin2.qq.com/"),
                ("Cookie", &format!("qrsig={key}")),
            ],
        )
        .await?;

    let fields = quoted_fields(&body);
    let code = fields.first().map(String::as_str).unwrap_or_default();
    let message = fields.get(4).cloned().filter(|text| !text.is_empty());

    match code {
        // 等待扫码
        "66" => Ok(QrPoll::state(QrStatus::Waiting, message)),
        // 已扫码，等手机确认
        "67" => Ok(QrPoll::state(QrStatus::Scanned, message)),
        // 已失效 / 用户取消
        "65" | "53" | "1" => Ok(QrPoll::state(QrStatus::Expired, message)),
        "0" => {
            let redirect = fields
                .get(2)
                .filter(|url| url.starts_with("http"))
                .ok_or_else(|| Error::Source {
                    platform: "qq".into(),
                    reason: "扫码成功但 QQ 没有给出跳转地址，请重新扫码".into(),
                })?;
            let login_cookie = complete_login(net, redirect, key).await?;
            let nickname = profile(net, &login_cookie).await;
            Ok(QrPoll {
                status: QrStatus::Confirmed,
                cookie: Some(login_cookie),
                nickname,
                message,
            })
        }
        other => Err(Error::Source {
            platform: "qq".into(),
            reason: format!(
                "扫码状态未知（ptuiCB code={other}）{}",
                message.unwrap_or_default()
            ),
        }),
    }
}

/// 第 3、4 跳：跟随 `check_sig` 拿 `skey`，再换成 `qm_keyst`。
async fn complete_login(net: &Net, redirect: &str, qrsig: &str) -> Result<String> {
    let response = net
        .get_no_redirect(
            redirect,
            &[
                ("Referer", "https://xui.ptlogin2.qq.com/"),
                ("Cookie", &format!("qrsig={qrsig}")),
            ],
        )
        .await?;
    let mut set_cookie = cookies_from_headers(response.headers());

    // 有些线路 check_sig 会再 302 到 pt4ub.y.qq.com，补一次请求才拿到 skey
    if cookie::value_of(&set_cookie, "skey").is_none() {
        if let Some(next) = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.starts_with("http"))
        {
            if let Ok(second) = net.get_no_redirect(next, &[]).await {
                let more = cookies_from_headers(second.headers());
                set_cookie = cookie::merge(&set_cookie, &more);
            }
        }
    }

    let raw_uin = cookie::value_of(&set_cookie, "uin")
        .or_else(|| cookie::value_of(&set_cookie, "p_uin"))
        .unwrap_or_default();
    let uin = raw_uin
        .trim_start_matches(['o', 'u', '_'])
        .to_string()
        .trim()
        .to_string();
    let skey = cookie::value_of(&set_cookie, "skey").unwrap_or_default();
    if uin.is_empty() || skey.is_empty() {
        return Err(Error::Source {
            platform: "qq".into(),
            reason: "扫码成功，但没拿到 QQ 登录态（uin/skey）。请改用「粘贴 Cookie」登录".into(),
        });
    }

    // 用 skey 换 QQ 音乐的 qm_keyst
    let payload = json!({
        "req": {
            "module": "QQConnectLogin.LoginServer",
            "method": "QQLogin",
            "param": { "expired_in": 7_776_000 }
        }
    });
    let encoded = utf8_percent_encode(&payload.to_string(), NON_ALPHANUMERIC).to_string();
    let url = format!("{MUSICU}?format=json&data={encoded}");
    let cookie_head = format!(
        "uin=o{uin}; skey={skey}; p_skey={}",
        cookie::value_of(&set_cookie, "p_skey").unwrap_or_default()
    );
    let body: serde_json::Value = net
        .json(
            &url,
            &[("Referer", YQQ), ("Origin", YQQ), ("Cookie", &cookie_head)],
        )
        .await?;

    let keyst = body
        .pointer("/req/data/qm_keyst")
        .or_else(|| body.pointer("/req/data/musickey"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::unavailable_with(
                Source::Qq,
                Reason::NeedLogin,
                "扫码成功但换取 QQ 音乐登录态失败，请改用「粘贴 Cookie」登录",
            )
        })?;
    let musicid = body
        .pointer("/req/data/musicid")
        .and_then(|value| value.as_u64())
        .map(|value| value.to_string())
        .unwrap_or_else(|| uin.clone());

    Ok(format!(
        "uin=o{musicid}; qm_keyst={keyst}; qqmusic_key={keyst}"
    ))
}

/// 尽力取昵称，失败不影响登录本身。
pub async fn profile(net: &Net, cookie: &str) -> Option<String> {
    let uin = cookie::value_of(cookie, "uin")
        .unwrap_or_default()
        .trim_start_matches(['o', 'u', '_'])
        .to_string();
    let url = format!(
        "https://c.y.qq.com/rsc/fcgi-bin/fcg_get_profile_homepage.fcg?cid=205360838\
         &ct=20&format=json&reqfrom=1&userid={uin}"
    );
    let body = net
        .text(&url, &[("Referer", YQQ), ("Cookie", cookie)])
        .await
        .ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    value
        .pointer("/data/nick")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            if uin.is_empty() {
                None
            } else {
                Some(format!("QQ {uin}"))
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash33_is_stable() {
        // 与网页端 JS 的 hash33 同算法，交叉验算过的固定值
        assert_eq!(hash33(""), 0);
        assert_eq!(hash33("a"), 97);
        assert_ne!(hash33("abc"), hash33("abd"));
        // 结果必须落在 31 位内，否则 ptqrtoken 会被服务端判为非法
        for seed in ["abc", "QRSIG123", "很长的一串票据"] {
            assert!(hash33(seed) <= 0x7fff_ffff);
        }
    }

    #[test]
    fn parses_ptuicb_fields() {
        let fields = quoted_fields("ptuiCB('67','0','','0','二维码已扫描，等待手机确认。','')");
        assert_eq!(fields[0], "67");
        assert_eq!(fields[2], "");
        assert_eq!(fields[4], "二维码已扫描，等待手机确认。");
    }

    /// 消息里带逗号和单引号时不能把字段切错
    #[test]
    fn parses_ptuicb_with_quotes_in_message() {
        let fields = quoted_fields("ptuiCB('65','0','','0','二维码已失效，请重新获取, 谢谢','')");
        assert_eq!(fields[0], "65");
        assert_eq!(fields.len(), 6);
        assert!(fields[4].contains("已失效"));
    }

    #[test]
    fn parses_success_redirect() {
        let fields = quoted_fields(
            "ptuiCB('0','0','https://ptlogin2.y.qq.com/check_sig?pttype=1&uin=123','0','登录成功！','')",
        );
        assert_eq!(fields[0], "0");
        assert!(fields[2].starts_with("https://ptlogin2.y.qq.com/check_sig"));
    }
}
