//! QQ 音乐扫码登录。
//!
//! 走的是 QQ 音乐**自己的**登录接口，而不是 QQ 互联（`ptlogin2`）那一套：
//!
//! 1. `music.login.LoginServer/CreateQRCode` —— 直接返回 `qrcodeID` 与一张
//!    `data:image/png;base64,...` 的二维码位图，有效期 15 分钟。
//! 2. `wss://mu.y.qq.com/ws/handshake` —— 以 MQTT 5 长连接订阅
//!    `management.qrcode_login/{qrcodeID}`，服务端把「已扫码 / 已取消 / 超时 /
//!    拿到令牌」作为 PUBLISH 事件推过来，见 [`super::mqtt`]。
//! 3. `music.login.LoginServer/Login` —— 用事件里的 `qqmusic_key` 换正式的
//!    `musickey`，同时返回数字 `musicid`。这两个值就是后续取下载链接的登录态。
//!
//! 为什么不用 `ptlogin2`：那条路要用 `skey` 再换一次 `qm_keyst`，多一次失败点，
//! 且换出来的凭据过一段时间就会失效。这里拿到的 `musickey` 是 QQ 音乐自己的
//! token，配上 `uin` 直接作为 `CgiGetVkey` 的 `authst` 参数即可，不需要拼 Cookie。
//!
//! 为什么不轮询 HTTP 状态：`GetQRCodeStatus` 实测在任何时刻都返回
//! `status: "scaned"` 且 `scanUser.userID` 恒为 0，是占位值，无法反映真实扫码状态。

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use crate::error::{Error, Result};
use crate::login::mqtt::{self, Frame};
use crate::login::{QrPoll, QrStatus, QrTicket};
use crate::net::Net;

const MUSICU: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const YQQ: &str = "https://y.qq.com/";
const WS_HOST: &str = "mu.y.qq.com";
const WS_PATH: &str = "/ws/handshake";
/// 二维码有效期，与接口返回的 `expiresIn` 一致
const QR_TTL_SECS: u64 = 900;
/// 客户端标识，服务端按设备版本决定下发哪些能力
const APP_CT: &str = "11";
const APP_CV: &str = "13020508";
const KEEP_ALIVE: u16 = 45;
/// 读超时：超过这个时间没收到任何报文就发一次 PINGREQ 探活
const READ_TIMEOUT: Duration = Duration::from_secs(20);
/// MQTT 节点重定向的最大次数
const MAX_REDIRECTS: usize = 3;

/// 一个二维码会话的当前状态。
struct Session {
    status: QrStatus,
    cookie: Option<String>,
    nickname: Option<String>,
    message: Option<String>,
}

impl Session {
    fn waiting() -> Self {
        Self {
            status: QrStatus::Waiting,
            cookie: None,
            nickname: None,
            message: None,
        }
    }
}

/// 全局会话表。`poll` 只读这里，不碰网络，因此前端可以按任意频率轮询。
fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn update(qrcode_id: &str, edit: impl FnOnce(&mut Session)) {
    if let Ok(mut map) = sessions().lock() {
        if let Some(session) = map.get_mut(qrcode_id) {
            edit(session);
        }
    }
}

/// 申请二维码，并在后台开始监听扫码事件。
pub async fn start(net: &Net) -> Result<QrTicket> {
    let data = api(
        net,
        "music.login.LoginServer",
        "CreateQRCode",
        json!({ "tmeAppID": "qqmusic", "ct": 11, "cv": 13020508 }),
        Value::Null,
    )
    .await?;

    let qrcode_id = data
        .get("qrcodeID")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Source {
            platform: "qq".into(),
            reason: "QQ 音乐没有返回二维码编号，请稍后重试".into(),
        })?
        .to_string();
    let qr_image = data
        .get("qrcode")
        .and_then(Value::as_str)
        .map(as_data_uri)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Source {
            platform: "qq".into(),
            reason: "QQ 音乐没有返回二维码图片，请稍后重试".into(),
        })?;

    if let Ok(mut map) = sessions().lock() {
        // 顺手清掉已经结束的旧会话，避免表无限增长
        map.retain(|_, session| matches!(session.status, QrStatus::Waiting | QrStatus::Scanned));
        map.insert(qrcode_id.clone(), Session::waiting());
    }

    let net = net.clone();
    let id = qrcode_id.clone();
    spawn(async move {
        run_session(net, id).await;
    });

    Ok(QrTicket {
        source: "qq".into(),
        key: qrcode_id,
        // 二维码是位图，没有可编码的内容
        qr_url: String::new(),
        expires_in: QR_TTL_SECS,
        qr_svg: None,
        qr_image: Some(qr_image),
    })
}

/// 查询扫码状态。纯读内存，不发请求。
pub async fn poll(_net: &Net, key: &str) -> Result<QrPoll> {
    let key = key.trim();
    if key.is_empty() {
        return Err(Error::Invalid("缺少二维码票据".into()));
    }
    let session = match sessions().lock() {
        Ok(map) => map.get(key).map(|session| QrPoll {
            status: session.status,
            cookie: session.cookie.clone(),
            nickname: session.nickname.clone(),
            message: session.message.clone(),
        }),
        Err(_) => None,
    };
    Ok(session.unwrap_or_else(|| {
        QrPoll::state(
            QrStatus::Expired,
            Some("二维码会话已结束，请重新获取".into()),
        )
    }))
}

/// 后台会话：连一次 MQTT，事件驱动地推进状态，直到成功 / 过期 / 出错。
async fn run_session(net: Net, qrcode_id: String) {
    match watch(&qrcode_id).await {
        Ok(Step::Cookies {
            music_id,
            music_key,
        }) => match finish_login(&net, music_id, &qrcode_id, &music_key).await {
            Ok((uin, musickey)) => {
                let cookie = format!("uin={uin}; authst={musickey}; qm_keyst={musickey}");
                update(&qrcode_id, |session| {
                    session.status = QrStatus::Confirmed;
                    session.cookie = Some(cookie);
                    session.nickname = Some(format!("QQ {uin}"));
                    session.message = None;
                });
            }
            Err(err) => {
                update(&qrcode_id, |session| {
                    session.status = QrStatus::Error;
                    session.message = Some(err.to_string());
                });
            }
        },
        Ok(Step::Expired) => update(&qrcode_id, |session| {
            session.status = QrStatus::Expired;
            session.message = Some("二维码已过期，请重新获取".into());
        }),
        Ok(Step::Canceled) => update(&qrcode_id, |session| {
            session.status = QrStatus::Expired;
            session.message = Some("已在手机上取消登录".into());
        }),
        Err(err) => update(&qrcode_id, |session| {
            session.status = QrStatus::Error;
            session.message = Some(err.to_string());
        }),
    }
}

/// `watch` 的三种正常结局（出错走 `Err`）。
enum Step {
    Cookies { music_id: u64, music_key: String },
    Expired,
    Canceled,
}

/// 建立 MQTT 会话并等待事件，必要时跟随节点重定向。
async fn watch(qrcode_id: &str) -> Result<Step> {
    let mut host = WS_HOST.to_string();
    let deadline = Instant::now() + Duration::from_secs(QR_TTL_SECS + 30);
    for _ in 0..MAX_REDIRECTS {
        match once(&host, qrcode_id, deadline).await? {
            StepOrMove::Step(step) => return Ok(step),
            StepOrMove::Move(next) => host = next,
        }
    }
    Err(Error::Source {
        platform: "qq".into(),
        reason: "MQTT 节点重定向次数超限，请重试".into(),
    })
}

enum StepOrMove {
    Step(Step),
    Move(String),
}

/// 一次完整的「连接 → 订阅 → 读事件」。
async fn once(host: &str, qrcode_id: &str, deadline: Instant) -> Result<StepOrMove> {
    let url = format!("wss://{host}{WS_PATH}");
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|err| Error::Source {
            platform: "qq".into(),
            reason: format!("构造 WebSocket 请求失败：{err}"),
        })?;
    request
        .headers_mut()
        .insert("Origin", "https://y.qq.com".parse().unwrap());

    let (mut ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|err| Error::Source {
            platform: "qq".into(),
            reason: format!("连接 QQ 音乐登录服务失败：{err}"),
        })?;

    let connect = mqtt::build_connect(
        &client_id(),
        KEEP_ALIVE,
        &[
            ("tmeAppID", "qqmusic"),
            ("business", "management"),
            ("hashTag", qrcode_id),
            ("clientTag", "management.user"),
            ("userID", qrcode_id),
        ],
    );
    send(&mut ws, connect).await?;

    // ---- 等 CONNACK ----
    let mut buffer = Vec::new();
    let mut subscribed = false;
    let sub_pkid: u16 = 1;
    loop {
        if Instant::now() > deadline {
            return Ok(StepOrMove::Step(Step::Expired));
        }
        let frame = next_frame(&mut ws, &mut buffer).await?;
        match frame {
            Some(Frame::ConnAck {
                code,
                server_reference,
            }) => {
                if code == 0 {
                    let packet = mqtt::build_subscribe(
                        sub_pkid,
                        &format!("management.qrcode_login/{qrcode_id}"),
                        &[("authorization", "tmelogin"), ("pubsub", "unicast")],
                    );
                    send(&mut ws, packet).await?;
                } else if code == mqtt::REASON_USE_ANOTHER_SERVER
                    || code == mqtt::REASON_SERVER_MOVED
                {
                    // 服务端让我们换节点，按 server_reference 重连
                    return Ok(StepOrMove::Move(
                        server_reference.unwrap_or_else(|| WS_HOST.to_string()),
                    ));
                } else {
                    return Err(Error::Source {
                        platform: "qq".into(),
                        reason: format!("QQ 音乐登录服务拒绝连接（原因码 {code}）"),
                    });
                }
            }
            Some(Frame::SubAck { pkid }) => {
                if pkid == sub_pkid {
                    subscribed = true;
                }
            }
            Some(Frame::Publish(publish)) => {
                // 订阅完成前也可能已经推到事件，照常处理
                if let Some(step) = handle_publish(&publish)? {
                    return Ok(StepOrMove::Step(step));
                }
            }
            Some(Frame::Other(0xc0)) => {
                // 服务端探活，回一个 PINGRESP
                send(&mut ws, vec![0xd0, 0x00]).await?;
            }
            Some(Frame::Other(_)) | None => {}
        }
        if subscribed {
            break;
        }
    }

    // ---- 订阅完成，等登录事件 ----
    loop {
        if Instant::now() > deadline {
            return Ok(StepOrMove::Step(Step::Expired));
        }
        match next_frame(&mut ws, &mut buffer).await? {
            Some(Frame::Publish(publish)) => {
                if let Some(step) = handle_publish(&publish)? {
                    return Ok(StepOrMove::Step(step));
                }
            }
            Some(Frame::Other(0xc0)) => send(&mut ws, vec![0xd0, 0x00]).await?,
            Some(Frame::ConnAck { .. }) | Some(Frame::SubAck { .. }) | Some(Frame::Other(_)) => {}
            None => {}
        }
    }
}

/// 把一个 PUBLISH 事件翻译成状态推进。
fn handle_publish(publish: &mqtt::Publish) -> Result<Option<Step>> {
    let kind = publish.prop("type").unwrap_or_default();
    match kind {
        "scanned" => {
            // 只改状态，不结束会话
            Ok(None)
        }
        "timeout" => Ok(Some(Step::Expired)),
        "canceled" => Ok(Some(Step::Canceled)),
        "loginFailed" => Err(Error::Source {
            platform: "qq".into(),
            reason: "QQ 音乐返回登录失败，请在手机上重新确认".into(),
        }),
        "cookies" => {
            let value: Value = serde_json::from_slice(&publish.payload)
                .map_err(|err| Error::decode(format!("解析扫码事件失败：{err}")))?;
            let cookies = value
                .get("cookies")
                .and_then(Value::as_object)
                .ok_or_else(|| Error::decode("扫码事件里没有 cookies 字段"))?;
            let music_key = cookie_value(cookies, "qqmusic_key")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| Error::Source {
                    platform: "qq".into(),
                    reason: "扫码成功但没有拿到登录令牌，请重试".into(),
                })?;
            let music_id = cookie_value(cookies, "qqmusic_uin")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            Ok(Some(Step::Cookies {
                music_id,
                music_key,
            }))
        }
        _ => Ok(None),
    }
}

/// 用扫码事件里的 `qqmusic_key` 换正式凭据。
async fn finish_login(
    net: &Net,
    music_id: u64,
    qrcode_id: &str,
    token: &str,
) -> Result<(String, String)> {
    let data = api(
        net,
        "music.login.LoginServer",
        "Login",
        json!({
            "musicid": music_id,
            "qrCodeID": qrcode_id,
            "token": token,
        }),
        json!({ "tmeLoginType": "6" }),
    )
    .await?;

    let musickey = data
        .get("musickey")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Source {
            platform: "qq".into(),
            reason: "QQ 音乐没有返回登录令牌（musickey），请重试".into(),
        })?
        .to_string();
    let uin = data
        .get("str_musicid")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            data.get("musicid")
                .and_then(Value::as_u64)
                .map(|v| v.to_string())
        })
        .unwrap_or_else(|| music_id.to_string());
    Ok((uin, musickey))
}

/// 从 PUBLISH 事件里读 cookie，兼容字符串 / 数字 / `{value: ...}` 三种形态。
fn cookie_value(cookies: &Map<String, Value>, key: &str) -> Option<String> {
    let value = cookies.get(key)?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    value.get("value").and_then(|inner| {
        inner
            .as_str()
            .map(str::to_string)
            .or_else(|| inner.as_u64().map(|v| v.to_string()))
    })
}

/// 调 QQ 音乐的统一接口，返回指定模块的 `data` 字段。
async fn api(
    net: &Net,
    module: &str,
    method: &str,
    param: Value,
    comm_extra: Value,
) -> Result<Value> {
    let mut comm = json!({
        "ct": APP_CT,
        "cv": APP_CV,
        "v": APP_CV,
        "tmeAppID": "qqmusic",
        "format": "json",
        "inCharset": "utf-8",
        "outCharset": "utf-8",
    });
    if let (Some(target), Some(extra)) = (comm.as_object_mut(), comm_extra.as_object()) {
        for (key, value) in extra {
            target.insert(key.clone(), value.clone());
        }
    }

    let request_key = format!("{module}.{method}");
    let mut body = Map::new();
    body.insert("comm".into(), comm);
    body.insert(
        request_key.clone(),
        json!({ "module": module, "method": method, "param": param }),
    );

    let response = net
        .client()
        .post(MUSICU)
        .header("Referer", YQQ)
        .header("Origin", YQQ)
        .json(&Value::Object(body))
        .send()
        .await?;
    let text = response.text().await?;
    let value: Value = serde_json::from_str(&text).map_err(|err| {
        let head: String = text.chars().take(160).collect();
        Error::decode(format!("{err} <- {head}"))
    })?;

    let node = value
        .get(&request_key)
        .ok_or_else(|| Error::decode(format!("返回里没有 {request_key}（可能被风控拦截）")))?;
    let code = node.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 0 {
        let msg = node.get("msg").and_then(Value::as_str).unwrap_or_default();
        return Err(Error::Source {
            platform: "qq".into(),
            reason: format!("{module}/{method} 返回 {code} {msg}")
                .trim()
                .to_string(),
        });
    }
    Ok(node.get("data").cloned().unwrap_or(Value::Null))
}

/// 服务端下发的二维码已经带 `data:` 前缀时原样返回，否则补上。
fn as_data_uri(raw: &str) -> String {
    let raw = raw.trim();
    if raw.starts_with("data:image/") {
        raw.to_string()
    } else if raw.is_empty() {
        String::new()
    } else {
        format!("data:image/png;base64,{raw}")
    }
}

fn client_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut seed = [0u8; 2];
    let _ = getrandom::getrandom(&mut seed);
    format!("{millis}{}", u16::from_be_bytes(seed) % 10_000)
}

async fn send<S>(ws: &mut S, bytes: Vec<u8>) -> Result<()>
where
    S: SinkExt<Message> + Unpin,
    <S as futures_util::Sink<Message>>::Error: std::fmt::Display,
{
    ws.send(Message::Binary(bytes))
        .await
        .map_err(|err| Error::Source {
            platform: "qq".into(),
            reason: format!("登录通道写入失败：{err}"),
        })
}

/// 读下一个 MQTT 帧。超时就发 PINGREQ 并返回 `None`，让调用方继续等。
async fn next_frame<S>(ws: &mut S, buffer: &mut Vec<u8>) -> Result<Option<Frame>>
where
    S: StreamExt<Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>
        + SinkExt<Message>
        + Unpin,
    <S as futures_util::Sink<Message>>::Error: std::fmt::Display,
{
    loop {
        // 缓冲区里可能已经攒了完整的一帧（一个 WebSocket 帧里可能塞多个 MQTT 报文）
        if let Some((frame, used)) = mqtt::decode_frame(buffer) {
            buffer.drain(..used);
            return Ok(Some(frame));
        }
        match tokio::time::timeout(READ_TIMEOUT, ws.next()).await {
            Ok(Some(Ok(Message::Binary(data)))) => buffer.extend_from_slice(&data),
            Ok(Some(Ok(Message::Ping(payload)))) => {
                let _ = ws.send(Message::Pong(payload)).await;
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
                return Err(Error::Source {
                    platform: "qq".into(),
                    reason: "QQ 音乐登录通道被关闭，请重试".into(),
                })
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => {
                return Err(Error::Source {
                    platform: "qq".into(),
                    reason: format!("登录通道异常：{err}"),
                })
            }
            // 读超时属于正常：长连接上本来就可能长时间没有数据
            Err(_) => {
                let _ = ws.send(Message::Binary(vec![0xc0, 0x00])).await;
                return Ok(None);
            }
        }
    }
}

/// 在后台跑一个 future。
///
/// 与下载引擎同样的三级兜底：当前运行时 → 已有运行时句柄 → 独立线程自建运行时。
/// Tauri 的同步 command 运行在主线程、没有 Tokio 上下文，直接 `tokio::spawn` 会 panic。
fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(future);
        return;
    }
    std::thread::spawn(move || {
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt.block_on(future),
            Err(err) => eprintln!("Melodex: 无法创建登录运行时：{err}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_uri_is_not_double_prefixed() {
        let raw = "data:image/png;base64,iVBORw0KGgo=";
        assert_eq!(as_data_uri(raw), raw);
        assert_eq!(
            as_data_uri("iVBORw0KGgo="),
            "data:image/png;base64,iVBORw0KGgo="
        );
        assert!(as_data_uri("  ").is_empty());
    }

    #[test]
    fn reads_cookie_in_three_shapes() {
        let mut cookies = Map::new();
        cookies.insert("a".into(), json!("text"));
        cookies.insert("b".into(), json!(12345));
        cookies.insert("c".into(), json!({ "value": "nested" }));
        assert_eq!(cookie_value(&cookies, "a").as_deref(), Some("text"));
        assert_eq!(cookie_value(&cookies, "b").as_deref(), Some("12345"));
        assert_eq!(cookie_value(&cookies, "c").as_deref(), Some("nested"));
        assert_eq!(cookie_value(&cookies, "missing"), None);
    }

    #[test]
    fn parses_cookies_event() {
        let payload = br#"{"cookies":{"qqmusic_uin":"10086","qqmusic_key":"KEY123"}}"#;
        let publish = mqtt::Publish {
            topic: "management.qrcode_login/QID".into(),
            payload: payload.to_vec(),
            user_props: vec![("type".into(), "cookies".into())],
        };
        match handle_publish(&publish).unwrap() {
            Some(Step::Cookies {
                music_id,
                music_key,
            }) => {
                assert_eq!(music_id, 10086);
                assert_eq!(music_key, "KEY123");
            }
            _ => panic!("应解析出 Cookies"),
        }
    }

    #[test]
    fn empty_music_key_is_an_error() {
        let payload = br#"{"cookies":{"qqmusic_uin":"1","qqmusic_key":""}}"#;
        let publish = mqtt::Publish {
            topic: "t".into(),
            payload: payload.to_vec(),
            user_props: vec![("type".into(), "cookies".into())],
        };
        assert!(handle_publish(&publish).is_err());
    }

    #[test]
    fn scanned_event_does_not_end_session() {
        let publish = mqtt::Publish {
            topic: "t".into(),
            payload: Vec::new(),
            user_props: vec![("type".into(), "scanned".into())],
        };
        assert!(handle_publish(&publish).unwrap().is_none());
    }

    #[test]
    fn timeout_and_cancel_are_terminal() {
        for (kind, expect_cancel) in [("timeout", false), ("canceled", true)] {
            let publish = mqtt::Publish {
                topic: "t".into(),
                payload: Vec::new(),
                user_props: vec![("type".into(), kind.into())],
            };
            match handle_publish(&publish).unwrap() {
                Some(Step::Expired) => assert!(!expect_cancel),
                Some(Step::Canceled) => assert!(expect_cancel),
                _ => panic!("事件 {kind} 应结束会话"),
            }
        }
    }

    #[test]
    fn unknown_event_is_ignored() {
        let publish = mqtt::Publish {
            topic: "t".into(),
            payload: Vec::new(),
            user_props: vec![("type".into(), "somethingElse".into())],
        };
        assert!(handle_publish(&publish).unwrap().is_none());
    }

    #[test]
    fn client_id_is_numeric_and_unique_enough() {
        let a = client_id();
        let b = client_id();
        assert!(a.chars().all(|c| c.is_ascii_digit()));
        assert_ne!(a, b);
    }

    #[test]
    fn poll_without_session_reports_expired() {
        let poll = poll_inner("不存在的二维码");
        assert_eq!(poll.status, QrStatus::Expired);
        assert!(poll.message.is_some());
    }

    /// `poll` 需要 `Net`，这里只测纯内存那段逻辑
    fn poll_inner(key: &str) -> QrPoll {
        let map = sessions().lock().unwrap();
        map.get(key)
            .map(|session| QrPoll {
                status: session.status,
                cookie: session.cookie.clone(),
                nickname: session.nickname.clone(),
                message: session.message.clone(),
            })
            .unwrap_or_else(|| {
                QrPoll::state(
                    QrStatus::Expired,
                    Some("二维码会话已结束，请重新获取".into()),
                )
            })
    }
}
