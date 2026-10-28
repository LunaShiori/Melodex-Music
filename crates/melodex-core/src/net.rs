use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

struct Clients {
    client: reqwest::Client,
    /// 不跟随重定向的客户端。扫码登录的 `check_sig` 一跳必须读到原始响应头，
    /// 否则 `Set-Cookie` 会在自动跳转中被丢掉。
    plain: reqwest::Client,
}

/// 所有音源共用的 HTTP 客户端。超时与连接池集中在这里，
/// 各平台只负责拼请求、传 Referer / Cookie。
///
/// 代理可以在运行时换（设置页保存后调 [`Net::set_proxy`]），
/// 所以客户端装在 `Arc<RwLock<_>>` 里，克隆出来的 Net 共享同一份——
/// 各平台手里拿着的是 `Net` 的克隆，换了代理会立刻生效。
#[derive(Clone)]
pub struct Net {
    clients: Arc<RwLock<Clients>>,
    proxy: Arc<RwLock<String>>,
}

impl Net {
    pub fn new() -> Result<Self> {
        Self::with_proxy("")
    }

    /// 建一套客户端。`proxy` 传空字符串表示直连。
    ///
    /// 代理格式不认识时不会让启动失败：直接当直连用（用户改设置页就能修）。
    pub fn with_proxy(proxy: &str) -> Result<Self> {
        let proxy = proxy.trim().to_string();
        let clients = match build(&proxy) {
            Ok(clients) => clients,
            // 地址不合法不该让整个应用起不来：退回直连，设置页里仍看得见用户填的原值。
            Err(_) => build("")?,
        };
        Ok(Self {
            clients: Arc::new(RwLock::new(clients)),
            proxy: Arc::new(RwLock::new(proxy)),
        })
    }

    /// 换代理。成功返回 `true`；格式无效时保持原样并返回 `false`。
    pub fn set_proxy(&self, proxy: &str) -> bool {
        let proxy = proxy.trim().to_string();
        match build(&proxy) {
            Ok(next) => {
                *self.clients.write().expect("net lock poisoned") = next;
                *self.proxy.write().expect("net lock poisoned") = proxy;
                true
            }
            Err(_) => false,
        }
    }

    pub fn proxy(&self) -> String {
        self.proxy.read().expect("net lock poisoned").clone()
    }

    pub fn client(&self) -> reqwest::Client {
        self.clients
            .read()
            .expect("net lock poisoned")
            .client
            .clone()
    }

    fn plain_client(&self) -> reqwest::Client {
        self.clients
            .read()
            .expect("net lock poisoned")
            .plain
            .clone()
    }

    pub async fn text(&self, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        Ok(self.get(url, headers).await?.text().await?)
    }

    /// 返回完整响应，用于需要读响应头的场景（如扫码登录的 `Set-Cookie`）。
    pub async fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<reqwest::Response> {
        let mut req = self.client().get(url);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Error::Source {
                platform: "http".into(),
                reason: format!("HTTP {status} <- {}", trim_url(url)),
            });
        }
        Ok(resp)
    }

    /// 不跟随重定向的 GET。
    ///
    /// QQ 扫码登录的最后一跳是关键：`check_sig` 这个 302 响应本身带着
    /// `uin` / `skey` / `p_skey` 三个 Cookie，一旦让客户端自动跟随，
    /// 中间这一跳的 `Set-Cookie` 就取不到了。
    pub async fn get_no_redirect(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<reqwest::Response> {
        let mut req = self.plain_client().get(url);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let resp = req.send().await?;
        let status = resp.status();
        // 3xx 在这里是预期结果，只有真正的错误码才算失败
        if status.is_client_error() || status.is_server_error() {
            return Err(Error::Source {
                platform: "http".into(),
                reason: format!("HTTP {status} <- {}", trim_url(url)),
            });
        }
        Ok(resp)
    }

    pub async fn json<T: DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let body = self.text(url, headers).await?;
        serde_json::from_str(&body)
            .map_err(|err| Error::decode(format!("{} <- {}", err, snippet(&body))))
    }
}

/// 按代理地址建一套客户端。空字符串 = 直连。
fn build(proxy: &str) -> Result<Clients> {
    let mut client = reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .pool_idle_timeout(Duration::from_secs(90));
    let mut plain = reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none());

    if !proxy.is_empty() {
        if !scheme_supported(proxy) {
            return Err(Error::Source {
                platform: "proxy".into(),
                reason: format!("只支持 http/https/socks5/socks4 代理：{proxy}"),
            });
        }
        let parsed = reqwest::Proxy::all(proxy)?;
        client = client.proxy(parsed.clone());
        plain = plain.proxy(parsed);
    }

    Ok(Clients {
        client: client.build()?,
        plain: plain.build()?,
    })
}

/// reqwest 对代理串比较宽容（"127.0.0.1:8080" 也认），
/// 但用户手填的中文/半截地址会被当成主机名，请求全挂。
/// 这里只放行明确支持的协议头，其余一律拒绝。
fn scheme_supported(proxy: &str) -> bool {
    let lower = proxy.to_ascii_lowercase();
    [
        "http://",
        "https://",
        "socks5h://",
        "socks5://",
        "socks4://",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn trim_url(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

fn snippet(body: &str) -> String {
    let head: String = body.chars().take(160).collect();
    head.replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_truncates() {
        let body = "x".repeat(500);
        assert_eq!(snippet(&body).len(), 160);
    }

    #[test]
    fn direct_net_has_no_proxy() {
        let net = Net::new().expect("直连客户端应当能建");
        assert_eq!(net.proxy(), "");
    }

    #[test]
    fn proxy_can_be_changed_at_runtime() {
        let net = Net::new().expect("直连客户端应当能建");
        assert!(net.set_proxy("http://127.0.0.1:7890"), "合法代理应当接受");
        assert_eq!(net.proxy(), "http://127.0.0.1:7890");
        // 换回直连
        assert!(net.set_proxy(""));
        assert_eq!(net.proxy(), "");
    }

    #[test]
    fn invalid_proxy_is_rejected_and_keeps_previous() {
        let net = Net::with_proxy("socks5://127.0.0.1:1080").expect("socks5 应当能建");
        assert!(!net.set_proxy("这不是一个地址"), "非法代理应当被拒绝");
        // 拒绝后仍然是原来那个：不会变直连，也不会变成半成品
        assert_eq!(net.proxy(), "socks5://127.0.0.1:1080");
    }
}
