//! Kugou QR login through its login-user service. No password or challenge bypass.
use crate::error::{Error, Result};
use crate::login::{QrPoll, QrStatus, QrTicket};
use crate::model::Source;
use crate::net::{Net, ProxyFor};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const TTL: u64 = 180;
#[derive(Serialize, Deserialize)]
struct Session {
    code: String,
    mid: String,
    issued: u64,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn signature(params: &BTreeMap<String, String>) -> String {
    let salt = "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt";
    let mut input = salt.to_string();
    for (key, value) in params {
        input.push_str(key);
        input.push('=');
        input.push_str(value);
    }
    input.push_str(salt);
    format!("{:x}", Md5::digest(input.as_bytes()))
}
async fn request(net: &Net, path: &str, mid: &str, extra: &[(&str, &str)]) -> Result<Value> {
    let mut params: BTreeMap<String, String> = [
        ("appid", "1014"),
        ("plat", "4"),
        ("srcappid", "2919"),
        ("clientver", "1000"),
        ("dfid", "-"),
        ("uuid", "-"),
        ("mid", mid),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect();
    params.insert("clienttime".into(), now().to_string());
    for (k, v) in extra {
        params.insert((*k).into(), (*v).into());
    }
    params.insert("signature".into(), signature(&params));
    let mut url = reqwest::Url::parse(&format!("https://login-user.kugou.com/v2/{path}"))
        .map_err(|e| Error::Invalid(e.to_string()))?;
    url.query_pairs_mut().extend_pairs(params.iter());
    let value: Value = net
        .json_src(
            Source::Kugou,
            ProxyFor::Search,
            url.as_str(),
            &[("Referer", "https://www.kugou.com/")],
        )
        .await?;
    if value["status"].as_i64() != Some(1) || value["error_code"].as_i64().unwrap_or(0) != 0 {
        return Err(Error::Source {
            platform: "kugou".into(),
            reason: format!(
                "扫码服务暂不可用（{}），请重试或使用官方网页登录",
                value["error_code"]
            ),
        });
    }
    Ok(value)
}
pub async fn start(net: &Net) -> Result<QrTicket> {
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).map_err(|e| Error::Invalid(e.to_string()))?;
    let mid = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let v = request(
        net,
        "qrcode",
        &mid,
        &[
            ("type", "1"),
            (
                "qrcode_txt",
                "https://h5.kugou.com/apps/loginQRCode/html/index.html?appid=1001&",
            ),
        ],
    )
    .await?;
    let code = v["data"]["qrcode"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::decode("酷狗未返回二维码票据"))?;
    let image = v["data"]["qrcode_img"]
        .as_str()
        .filter(|s| s.starts_with("data:image/png;base64,") && s.len() < 500_000)
        .ok_or_else(|| Error::decode("酷狗未返回有效二维码图片"))?;
    let key = serde_json::to_string(&Session {
        code: code.into(),
        mid,
        issued: now(),
    })
    .map_err(|e| Error::decode(e.to_string()))?;
    Ok(QrTicket {
        source: "kugou".into(),
        key,
        qr_url: String::new(),
        expires_in: TTL,
        qr_svg: None,
        qr_image: Some(image.into()),
    })
}
pub async fn poll(net: &Net, key: &str) -> Result<QrPoll> {
    if key.len() > 4096 {
        return Err(Error::Invalid("无效的扫码票据".into()));
    }
    let s: Session =
        serde_json::from_str(key).map_err(|_| Error::Invalid("无效的扫码票据".into()))?;
    if s.code.is_empty() || s.mid.len() != 32 || !s.mid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Invalid("无效的扫码票据".into()));
    }
    if now().saturating_sub(s.issued) >= TTL || s.issued > now() {
        return Ok(QrPoll::state(QrStatus::Expired, None));
    }
    let value = request(net, "get_userinfo_qrcode", &s.mid, &[("qrcode", &s.code)]).await?;
    parse_poll(&value["data"], &s.mid)
}
fn parse_poll(data: &Value, mid: &str) -> Result<QrPoll> {
    let status = match data["status"].as_i64() {
        Some(0) => QrStatus::Expired,
        Some(1) => QrStatus::Waiting,
        Some(2) => QrStatus::Scanned,
        Some(4) => QrStatus::Confirmed,
        _ => return Err(Error::decode("酷狗返回未知扫码状态，请重新扫码")),
    };
    if status != QrStatus::Confirmed {
        return Ok(QrPoll::state(status, None));
    }
    let token = data["token"]
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.bytes().all(
                    |b| matches!(b, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e),
                )
        })
        .ok_or_else(|| Error::decode("登录响应缺少有效令牌，请重新扫码"))?;
    let uid = data["userid"]
        .as_u64()
        .filter(|id| *id > 0)
        .map(|id| id.to_string())
        .or_else(|| {
            data["userid"]
                .as_str()
                .and_then(|id| id.parse::<u64>().ok())
                .filter(|id| *id > 0)
                .map(|id| id.to_string())
        })
        .ok_or_else(|| Error::decode("登录响应缺少用户标识，请重新扫码"))?;
    Ok(QrPoll {
        status,
        cookie: Some(format!("userid={uid}; token={token}; mid={mid}")),
        nickname: data["nickname"].as_str().map(str::to_owned),
        message: None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn states_and_missing_credentials() {
        for (n, status) in [
            (0, QrStatus::Expired),
            (1, QrStatus::Waiting),
            (2, QrStatus::Scanned),
        ] {
            let p = parse_poll(&serde_json::json!({"status":n}), "mid").unwrap();
            assert_eq!(p.status, status);
            assert!(p.cookie.is_none());
        }
        assert!(parse_poll(&serde_json::json!({"status":4}), "mid").is_err());
        assert!(parse_poll(&serde_json::json!({"status":3}), "mid").is_err());
        let p = parse_poll(
            &serde_json::json!({"status":4,"userid":"123","token":"abc_123"}),
            "mid",
        )
        .unwrap();
        assert_eq!(
            p.cookie.as_deref(),
            Some("userid=123; token=abc_123; mid=mid")
        );
        assert!(parse_poll(
            &serde_json::json!({"status":4,"userid":123,"token":"a; injected=x"}),
            "mid"
        )
        .is_err());
    }
}

#[cfg(test)]
mod live_tests {
    #[tokio::test]
    #[ignore = "contacts the official QR service; does not sign in"]
    async fn qr_service_waiting() {
        let net = crate::net::Net::with_proxy("").unwrap();
        let ticket = super::start(&net).await.unwrap();
        assert!(ticket
            .qr_image
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let result = super::poll(&net, &ticket.key).await.unwrap();
        assert_eq!(result.status, crate::login::QrStatus::Waiting);
        assert!(result.cookie.is_none());
    }
}
