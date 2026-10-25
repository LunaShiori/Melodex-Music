use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// 所有音源共用的 HTTP 客户端。超时与连接池集中在这里，
/// 各平台只负责拼请求、传 Referer / Cookie。
#[derive(Clone)]
pub struct Net {
    client: reqwest::Client,
    /// 不跟随重定向的客户端。扫码登录的 `check_sig` 一跳必须读到原始响应头，
    /// 否则 `Set-Cookie` 会在自动跳转中被丢掉。
    plain: reqwest::Client,
}

impl Net {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()?;
        let plain = reqwest::Client::builder()
            .user_agent(UA)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { client, plain })
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn text(&self, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        Ok(self.get(url, headers).await?.text().await?)
    }

    /// 返回完整响应，用于需要读响应头的场景（如扫码登录的 `Set-Cookie`）。
    pub async fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<reqwest::Response> {
        let mut req = self.client.get(url);
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
        let mut req = self.plain.get(url);
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
}
