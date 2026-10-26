use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use crate::error::{Error, Result};
use crate::model::{Lyric, PlayUrl, PlaylistDetail, SearchPage, Source, Variant};
use crate::net::Net;
use crate::quality::{self, Quality};

/// 一次搜索请求。音源的 offset 由适配器自行换算，避免各平台分页口径不同。
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub keyword: String,
    pub page: u32,
    pub limit: u32,
}

impl SearchQuery {
    pub fn new(keyword: impl Into<String>, page: u32, limit: u32) -> Self {
        Self {
            keyword: keyword.into(),
            page: page.max(1),
            limit: limit.clamp(1, 100),
        }
    }

    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.limit
    }
}

/// 用户自备凭据（各平台 Cookie）。仅用于读取用户自己账号的权限范围。
#[derive(Clone, Default)]
pub struct CredStore {
    inner: Arc<RwLock<HashMap<String, String>>>,
}

impl CredStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, source: Source, cookie: Option<String>) {
        let mut guard = self.inner.write().expect("cred store poisoned");
        match cookie {
            Some(value) if !value.trim().is_empty() => {
                guard.insert(source.id().to_string(), value.trim().to_string());
            }
            _ => {
                guard.remove(source.id());
            }
        }
    }

    pub fn cookie(&self, source: Source) -> Option<String> {
        self.inner
            .read()
            .expect("cred store poisoned")
            .get(source.id())
            .cloned()
    }
}

/// 音源适配器契约。新增音源只需实现该 trait 并注册进 [`Registry`]。
#[async_trait]
pub trait MusicPlatform: Send + Sync {
    fn source(&self) -> Source;

    async fn search(&self, query: &SearchQuery) -> Result<SearchPage>;

    /// 把一条记录解析成指定档位的直链。
    ///
    /// 只负责**一个**档位：取不到时返回 `Error::Unavailable`（通常是
    /// `Reason::QualityMissing`），由下载引擎按 [`quality::ladder`] 继续降级，
    /// 跨源兜底也由引擎统一处理——平台适配器不需要知道降级链长什么样。
    async fn resolve(&self, variant: &Variant, quality: &'static Quality) -> Result<PlayUrl>;

    async fn lyric(&self, _id: &str) -> Result<Option<Lyric>> {
        Ok(None)
    }

    async fn playlist(&self, _id: &str) -> Result<Option<PlaylistDetail>> {
        Ok(None)
    }

    /// 该音源支持的音质档位，从高到低。默认取 [`quality::table`] 里的静态表。
    fn qualities(&self) -> &'static [Quality] {
        quality::table(self.source())
    }

    /// 播放直链需要的额外请求头（`Referer` / `Origin`）。
    ///
    /// `<audio>` 的请求头由 WebView 决定，改不了；缺 Referer 时 QQ 音乐的 CDN
    /// 会 403，所以外壳拿这张表去跑 [`crate::stream`] 里的本地代理。
    fn stream_headers(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
}

/// 音源注册表：把 `Source` 映射到具体实现。
#[derive(Clone)]
pub struct Registry {
    platforms: Arc<HashMap<Source, Arc<dyn MusicPlatform>>>,
}

impl Registry {
    pub fn new(net: Net, creds: CredStore) -> Self {
        let mut map: HashMap<Source, Arc<dyn MusicPlatform>> = HashMap::new();
        map.insert(
            Source::Netease,
            Arc::new(crate::platforms::netease::Netease::new(
                net.clone(),
                creds.clone(),
            )),
        );
        map.insert(
            Source::Qq,
            Arc::new(crate::platforms::qq::QqMusic::new(
                net.clone(),
                creds.clone(),
            )),
        );
        map.insert(
            Source::Kuwo,
            Arc::new(crate::platforms::kuwo::Kuwo::new(net.clone(), creds)),
        );
        Self {
            platforms: Arc::new(map),
        }
    }

    pub fn get(&self, source: Source) -> Result<Arc<dyn MusicPlatform>> {
        self.platforms
            .get(&source)
            .cloned()
            .ok_or_else(|| Error::Source {
                platform: source.id().into(),
                reason: "音源未注册".into(),
            })
    }

    pub fn all(&self) -> Vec<Arc<dyn MusicPlatform>> {
        Source::ALL
            .iter()
            .filter_map(|s| self.platforms.get(s).cloned())
            .collect()
    }

    pub fn sources() -> &'static [Source] {
        &Source::ALL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_paginates() {
        let q = SearchQuery::new("晴天", 3, 30);
        assert_eq!(q.offset(), 60);
        let q = SearchQuery::new("x", 0, 500);
        assert_eq!(q.page, 1);
        assert_eq!(q.limit, 100);
    }

    #[test]
    fn cred_store_roundtrip() {
        let store = CredStore::new();
        assert!(store.cookie(Source::Qq).is_none());
        store.set(Source::Qq, Some(" a=1 ".into()));
        assert_eq!(store.cookie(Source::Qq).as_deref(), Some("a=1"));
        store.set(Source::Qq, Some("  ".into()));
        assert!(store.cookie(Source::Qq).is_none());
    }
}
