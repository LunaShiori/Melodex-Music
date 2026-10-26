//! 回环流代理：让 WebView 的 `<audio>` 用上「必须带 Referer」的直链。
//!
//! QQ 音乐、网易云的音乐 CDN 都要求请求带对应站点的 `Referer`，否则 403。
//! 下载走的是 [reqwest]（我们自己拼头），所以一直正常；播放走的是
//! `<audio src="...">`，请求头由 WebView 决定，我们插不进去——这正是
//! 「能下载、点播放却没有任何声音」的根因。
//!
//! 这里起一个只监听 `127.0.0.1` 的小 HTTP 服务：前端把原始直链交给它，
//! 它补上缺少的请求头转发，并把上游的状态码与 `Range` 语义原样透传，
//! 这样拖动进度条仍然能 seek，也不必把整个文件读进内存。
//!
//! 安全边界：
//! - 只绑回环地址，外网访问不到；
//! - 路径里带一次性随机 token，同机的其它进程猜不到就能读；
//! - 只允许 `Referer` / `Origin` 两种附加头，不能借它发任意请求头。

use std::time::Duration;

use futures_util::StreamExt;
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::net::UA;

/// 代理对外暴露的信息。`base` 形如 `http://127.0.0.1:53211`。
#[derive(Debug, Clone)]
pub struct StreamProxy {
    base: String,
    token: String,
}

impl StreamProxy {
    /// 把一条直链包成代理地址。`referer` 为空时只做透传（不改请求头）。
    pub fn wrap(&self, target: &str, referer: Option<&str>, origin: Option<&str>) -> String {
        let mut url = format!(
            "{}/p?t={}&u={}",
            self.base,
            self.token,
            utf8_percent_encode(target, NON_ALPHANUMERIC),
        );
        if let Some(value) = referer.filter(|value| !value.is_empty()) {
            url.push_str("&r=");
            url.push_str(&utf8_percent_encode(value, NON_ALPHANUMERIC).to_string());
        }
        if let Some(value) = origin.filter(|value| !value.is_empty()) {
            url.push_str("&o=");
            url.push_str(&utf8_percent_encode(value, NON_ALPHANUMERIC).to_string());
        }
        url
    }

    pub fn base(&self) -> &str {
        &self.base
    }
}

/// 创建代理并开始接受连接。调用方持有返回的句柄即可拼代理地址。
pub async fn spawn() -> std::io::Result<StreamProxy> {
    let listener = TcpListener::bind(("127.0.0.1", 0u16)).await?;
    let addr = listener.local_addr()?;
    let token = random_token();
    let proxy = StreamProxy {
        base: format!("http://{addr}"),
        token: token.clone(),
    };

    tokio::spawn(async move {
        // 拿不到客户端就不启动，避免每个连接都重试建连
        let Ok(client) = build_client() else { return };
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                continue;
            };
            let token = token.clone();
            let client = client.clone();
            tokio::spawn(async move {
                let _ = serve(socket, client, &token).await;
            });
        }
    });

    Ok(proxy)
}

/// 流式转发专用客户端：**不能有整体超时**，否则听到一半就被 reqwest 掐断。
fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(8))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
}

/// 探测直链能否被 `<audio>` 直接使用。
///
/// 只发 `Range: bytes=0-1`，拿到状态码就断开，不消耗用户流量。
pub async fn probe(url: &str, referer: Option<&str>, origin: Option<&str>) -> Probe {
    let Ok(client) = build_client() else {
        return Probe::Failed("无法创建 HTTP 客户端".into());
    };
    // 用 GET 而不是 HEAD：部分 CDN 对 HEAD 直接 405，用 HEAD 探测会误判成不可播
    match send(&client, url, Some("bytes=0-1"), referer, origin).await {
        Ok(status) if status.is_success() => Probe::Direct,
        Ok(status) => Probe::Failed(format!("HTTP {}", status.as_u16())),
        Err(err) => Probe::Failed(err.to_string()),
    }
}

/// 探测结果。
#[derive(Debug, Clone)]
pub enum Probe {
    /// 不带额外请求头也能播，直接用原链接（省掉代理中转）。
    Direct,
    /// 探测失败，附上拿到的状态码或网络错误。
    Failed(String),
}

/// 同时探测「裸链」与「带 Referer」。
///
/// 两个都是 `Range: bytes=0-1` 的小请求，并行发只花一次往返的时间；
/// 串行的话每次播放要多等一个 RTT，手机上感知很明显。
pub async fn probe_pair(url: &str, referer: Option<&str>, origin: Option<&str>) -> (Probe, Probe) {
    tokio::join!(probe(url, None, None), probe(url, referer, origin))
}

async fn send(
    client: &reqwest::Client,
    url: &str,
    range: Option<&str>,
    referer: Option<&str>,
    origin: Option<&str>,
) -> Result<reqwest::StatusCode, String> {
    let mut req = client.get(url);
    if let Some(value) = range {
        req = req.header("Range", value);
    }
    if let Some(value) = referer.filter(|v| !v.is_empty()) {
        req = req.header("Referer", value);
    }
    if let Some(value) = origin.filter(|v| !v.is_empty()) {
        req = req.header("Origin", value);
    }
    match req.send().await {
        Ok(resp) => Ok(resp.status()),
        Err(err) => Err(err.to_string()),
    }
}

async fn serve(mut socket: TcpStream, client: reqwest::Client, token: &str) -> std::io::Result<()> {
    let Some(head) = read_head(&mut socket).await? else {
        return Ok(());
    };
    let mut parts = head.lines().next().unwrap_or_default().split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();

    if method != "GET" && method != "HEAD" {
        return reply_text(&mut socket, 405, "只支持 GET / HEAD").await;
    }

    let Some(call) = decode_target(&target, token) else {
        return reply_text(&mut socket, 403, "代理链接无效").await;
    };

    // 浏览器的 media 请求会带 Range 做 seek，必须原样转发
    let range = header_value(&head, "range");
    let mut req = client
        .get(&call.url)
        .header("Range", range.unwrap_or("bytes=0-"));
    if let Some(value) = call.referer.as_deref() {
        req = req.header("Referer", value);
    }
    if let Some(value) = call.origin.as_deref() {
        req = req.header("Origin", value);
    }

    let resp = match req.send().await {
        Ok(resp) => resp,
        Err(err) => {
            let short: String = err.to_string().chars().take(160).collect();
            return reply_text(&mut socket, 502, &format!("上游不可达：{short}")).await;
        }
    };

    if !resp.status().is_success() {
        // 把上游状态码原样交给前端，播放器据此能说清是 403 还是 404
        let status = resp.status();
        return reply_text(&mut socket, status.as_u16(), status.as_str()).await;
    }

    let mut head_out = if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        String::from("HTTP/1.1 206 Partial Content\r\n")
    } else {
        String::from("HTTP/1.1 200 OK\r\n")
    };
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
        "etag",
        "last-modified",
    ] {
        if let Some(value) = resp.headers().get(name).and_then(|v| v.to_str().ok()) {
            head_out.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    // <audio> 跨源取媒体不检查 CORS，但带上更稳
    head_out.push_str("Access-Control-Allow-Origin: *\r\n");
    head_out.push_str("Cache-Control: no-store\r\n");
    head_out.push_str("Connection: close\r\n\r\n");
    socket.write_all(head_out.as_bytes()).await?;

    if method == "HEAD" {
        return socket.flush().await;
    }

    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        let Ok(chunk) = chunk else { break };
        if socket.write_all(&chunk).await.is_err() {
            // 用户拖走播放位置，前端会主动断开，属于正常情况
            break;
        }
    }
    let _ = socket.flush().await;
    Ok(())
}

struct Call {
    url: String,
    referer: Option<String>,
    origin: Option<String>,
}

/// 解析 `/p?t=<token>&u=<target>&r=<referer>&o=<origin>`。
///
/// 自己解析而不用现成的表单解析器：URL 里嵌套了完整的原始链接，
/// 任何二次编码/解码的差错都会表现成「404 但看不出为什么」。
fn decode_target(target: &str, token: &str) -> Option<Call> {
    let query = target.split_once('?')?.1;
    let mut url = None;
    let mut referer = None;
    let mut origin = None;
    let mut given = None;

    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let decoded = percent_decode_str(value).decode_utf8().ok()?.to_string();
        match key {
            "t" => given = Some(decoded),
            "u" => url = Some(decoded),
            "r" => referer = Some(decoded),
            "o" => origin = Some(decoded),
            _ => {}
        }
    }

    // 定长比较，避免 token 校验被时序侧信道绕过
    if !constant_eq(given.as_deref().unwrap_or_default(), token) {
        return None;
    }
    Some(Call {
        url: url?,
        referer,
        origin,
    })
}

fn constant_eq(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

/// 最多读 8 KB 请求头。`<audio>` 不会发 body，所以只处理请求行 + 头。
async fn read_head(socket: &mut TcpStream) -> std::io::Result<Option<String>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let read = socket.read(&mut chunk).await?;
        if read == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..read]);
        if buf.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(Some(String::from_utf8_lossy(&buf).into_owned()));
        }
        if buf.len() > 8192 {
            return Ok(None);
        }
    }
}

fn header_value(head: &str, wanted: &str) -> Option<&str> {
    head.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(wanted)
            .then(|| value.trim())
    })
}

async fn reply_text(socket: &mut TcpStream, code: u16, message: &str) -> std::io::Result<()> {
    let body = message.as_bytes();
    let head = format!(
        "HTTP/1.1 {code} {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status_text(code),
        body.len()
    );
    socket.write_all(head.as_bytes()).await?;
    socket.write_all(body).await?;
    socket.flush().await
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        206 => "Partial Content",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        502 => "Bad Gateway",
        _ => "Error",
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 16];
    // getrandom 是库里的既有依赖（凭据加密也用），失败时退回时间戳
    if getrandom::getrandom(&mut bytes).is_err() {
        return format!("{:x}", std::process::id() as u128);
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_roundtrip() {
        let proxy = StreamProxy {
            base: "http://127.0.0.1:1234".into(),
            token: "abc123".into(),
        };
        let raw = "http://ws.stream.qqmusic.qq.com/M500.mp3?vkey=1&guid=2&uin=0";
        let wrapped = proxy.wrap(raw, Some("https://y.qq.com/"), None);
        let path = wrapped.trim_start_matches("http://127.0.0.1:1234");
        let call = decode_target(path, "abc123").expect("应能解回原始请求");
        assert_eq!(call.url, raw);
        assert_eq!(call.referer.as_deref(), Some("https://y.qq.com/"));
        assert!(call.origin.is_none());
    }

    #[test]
    fn rejects_wrong_token() {
        let path = "/p?t=guess&u=http%3A%2F%2Fa.com";
        assert!(decode_target(path, "abc123").is_none());
    }

    #[test]
    fn rejects_missing_target() {
        assert!(decode_target("/p?t=abc123", "abc123").is_none());
        assert!(decode_target("/p?t=abc123&u=", "abc123").is_none());
    }

    #[test]
    fn token_is_hex_and_long_enough() {
        let token = random_token();
        assert!(token.len() >= 32, "{token}");
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let head = "GET /p HTTP/1.1\r\nHost: x\r\nRange: bytes=0-99\r\n\r\n";
        assert_eq!(header_value(head, "range"), Some("bytes=0-99"));
        assert_eq!(header_value(head, "Range"), Some("bytes=0-99"));
        assert_eq!(header_value(head, "cookie"), None);
    }

    #[test]
    fn constant_eq_compares_content() {
        assert!(constant_eq("abc", "abc"));
        assert!(!constant_eq("abc", "abd"));
        assert!(!constant_eq("abc", "ab"));
    }
}
