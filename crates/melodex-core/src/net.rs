use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::settings::ProxyScopes;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// 一次请求的用途。代理是否生效按这个分类判断——
/// 设置页可以让「搜索走直连、下载走代理」，也可以反过来。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyFor {
    /// 搜索、歌词、歌单、账号这类「在线浏览」请求
    Search,
    /// 取播放链接：vkey / song-url 之类的短请求
    Link,
    /// 音频文件传输（含封面）
    Download,
    /// 检查更新、下安装包
    Update,
}

impl ProxyScopes {
    /// 这个用途该不该走代理。
    fn allows(self, purpose: ProxyFor) -> bool {
        match purpose {
            ProxyFor::Search => self.search,
            ProxyFor::Link => self.link,
            ProxyFor::Download => self.download,
            ProxyFor::Update => self.update,
        }
    }
}

#[derive(Clone)]
struct Clients {
    client: reqwest::Client,
    /// 不跟随重定向的客户端。扫码登录的 `check_sig` 一跳必须读到原始响应头，
    /// 否则 `Set-Cookie` 会在自动跳转中被丢掉。
    plain: reqwest::Client,
}

/// 所有音源共用的 HTTP 客户端。超时与连接池集中在这里，
/// 各平台只负责拼请求、传 Referer / Cookie。
///
/// 代理与「代理用在哪几件事上」都可以在运行时改（设置页保存后调
/// [`Net::set_proxy`] / [`Net::set_scopes`]），所以状态装在 `Arc<RwLock<_>>` 里，
/// 克隆出来的 Net 共享同一份——各平台手里拿着的是 `Net` 的克隆，改完立刻生效。
#[derive(Clone)]
pub struct Net {
    /// 走代理的客户端；没填代理或地址非法时为 `None`（此时一律直连）。
    proxied: Arc<RwLock<Option<Clients>>>,
    /// 直连客户端，始终可用。
    direct: Clients,
    /// 用户填的原值（已 trim），非法也照样保留，设置页要能看见自己填了什么。
    proxy: Arc<RwLock<String>>,
    scopes: Arc<RwLock<ProxyScopes>>,
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
        let direct = build("")?;
        let proxied = if proxy.is_empty() {
            None
        } else {
            // 地址不合法不该让整个应用起不来：退回直连，设置页里仍看得见用户填的原值。
            build(&proxy).ok()
        };
        Ok(Self {
            proxied: Arc::new(RwLock::new(proxied)),
            direct,
            proxy: Arc::new(RwLock::new(proxy)),
            scopes: Arc::new(RwLock::new(ProxyScopes::default())),
        })
    }

    /// 换代理。成功返回 `true`；格式无效时保持原样并返回 `false`。
    pub fn set_proxy(&self, proxy: &str) -> bool {
        let proxy = proxy.trim().to_string();
        if proxy.is_empty() {
            *self.proxied.write().expect("net lock poisoned") = None;
            *self.proxy.write().expect("net lock poisoned") = proxy;
            return true;
        }
        match build(&proxy) {
            Ok(next) => {
                *self.proxied.write().expect("net lock poisoned") = Some(next);
                *self.proxy.write().expect("net lock poisoned") = proxy;
                true
            }
            Err(_) => false,
        }
    }

    /// 改「代理用在哪几件事上」。四条开关互相独立，改完立刻对下一次请求生效。
    pub fn set_scopes(&self, scopes: ProxyScopes) {
        *self.scopes.write().expect("net lock poisoned") = scopes;
    }

    pub fn scopes(&self) -> ProxyScopes {
        *self.scopes.read().expect("net lock poisoned")
    }

    pub fn proxy(&self) -> String {
        self.proxy.read().expect("net lock poisoned").clone()
    }

    /// 这一类请求会不会走代理：填了合法代理，且对应的开关是开的。
    pub fn uses_proxy(&self, purpose: ProxyFor) -> bool {
        if !self.scopes().allows(purpose) {
            return false;
        }
        self.proxied.read().expect("net lock poisoned").is_some()
    }

    pub fn client_for(&self, purpose: ProxyFor) -> reqwest::Client {
        match self.pick(purpose) {
            Some(clients) => clients.client.clone(),
            None => self.direct.client.clone(),
        }
    }

    fn plain_for(&self, purpose: ProxyFor) -> reqwest::Client {
        match self.pick(purpose) {
            Some(clients) => clients.plain.clone(),
            None => self.direct.plain.clone(),
        }
    }

    fn pick(&self, purpose: ProxyFor) -> Option<Clients> {
        if !self.scopes().allows(purpose) {
            return None;
        }
        self.proxied.read().expect("net lock poisoned").clone()
    }

    pub fn client(&self) -> reqwest::Client {
        self.client_for(ProxyFor::Search)
    }

    pub async fn text(&self, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        self.text_for(ProxyFor::Search, url, headers).await
    }

    pub async fn text_for(
        &self,
        purpose: ProxyFor,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<String> {
        Ok(self.get_for(purpose, url, headers).await?.text().await?)
    }

    /// 返回完整响应，用于需要读响应头的场景（如扫码登录的 `Set-Cookie`）。
    pub async fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<reqwest::Response> {
        self.get_for(ProxyFor::Search, url, headers).await
    }

    pub async fn get_for(
        &self,
        purpose: ProxyFor,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<reqwest::Response> {
        let mut req = self.client_for(purpose).get(url);
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
        let mut req = self.plain_for(ProxyFor::Search).get(url);
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
        self.json_for(ProxyFor::Search, url, headers).await
    }

    pub async fn json_for<T: DeserializeOwned>(
        &self,
        purpose: ProxyFor,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let body = self.text_for(purpose, url, headers).await?;
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
        // 没填代理时，再开阔的开关也不该让请求「走代理」
        for purpose in [
            ProxyFor::Search,
            ProxyFor::Link,
            ProxyFor::Download,
            ProxyFor::Update,
        ] {
            assert!(!net.uses_proxy(purpose), "{purpose:?} 不该走代理");
        }
    }

    #[test]
    fn proxy_can_be_changed_at_runtime() {
        let net = Net::new().expect("直连客户端应当能建");
        assert!(net.set_proxy("http://127.0.0.1:7890"), "合法代理应当接受");
        assert_eq!(net.proxy(), "http://127.0.0.1:7890");
        assert!(net.uses_proxy(ProxyFor::Search));
        // 换回直连
        assert!(net.set_proxy(""));
        assert_eq!(net.proxy(), "");
        assert!(!net.uses_proxy(ProxyFor::Search));
    }

    #[test]
    fn invalid_proxy_is_rejected_and_keeps_previous() {
        let net = Net::with_proxy("socks5://127.0.0.1:1080").expect("socks5 应当能建");
        assert!(!net.set_proxy("这不是一个地址"), "非法代理应当被拒绝");
        // 拒绝后仍然是原来那个：不会变直连，也不会变成半成品
        assert_eq!(net.proxy(), "socks5://127.0.0.1:1080");
        assert!(net.uses_proxy(ProxyFor::Search));
    }

    #[test]
    fn default_scopes_send_everything_through_proxy() {
        let net = Net::with_proxy("http://127.0.0.1:7890").expect("http 代理应当能建");
        assert_eq!(net.scopes(), ProxyScopes::default());
        for purpose in [
            ProxyFor::Search,
            ProxyFor::Link,
            ProxyFor::Download,
            ProxyFor::Update,
        ] {
            assert!(net.uses_proxy(purpose), "{purpose:?} 默认应当走代理");
        }
    }

    #[test]
    fn scopes_only_route_the_ticked_purposes() {
        let net = Net::with_proxy("http://127.0.0.1:7890").expect("http 代理应当能建");
        net.set_scopes(ProxyScopes {
            search: false,
            link: false,
            download: true,
            update: false,
        });
        assert!(!net.uses_proxy(ProxyFor::Search), "搜索关掉后应当直连");
        assert!(!net.uses_proxy(ProxyFor::Link));
        assert!(!net.uses_proxy(ProxyFor::Update));
        assert!(
            net.uses_proxy(ProxyFor::Download),
            "只勾了下载，它就还走代理"
        );
    }

    #[test]
    fn clearing_proxy_overrides_scopes() {
        let net = Net::with_proxy("http://127.0.0.1:7890").expect("http 代理应当能建");
        assert!(net.set_proxy(""));
        assert!(!net.uses_proxy(ProxyFor::Search), "清空代理后一律直连");
    }
}
