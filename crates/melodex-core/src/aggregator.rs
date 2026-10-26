use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use tokio::time::timeout;

use crate::model::{normalize_title, AggregatedPage, Source, SourceReport, Track, Variant};
use crate::platform::{MusicPlatform, Registry, SearchQuery};

/// 跨源聚合器：并发扇出到各音源，合并同一首歌，再按相关度排序。
///
/// 设计要点：
/// - 单源失败或超时只影响它自己（`SourceReport` 会如实上报），不拖垮整次搜索；
/// - 结果按音源轮转合并，避免「前 30 条全是同一个源」；
/// - 去重以「标题 + 首位歌手 + 时长」为准，容忍各源元数据的小幅偏差。
pub struct Aggregator {
    registry: Registry,
    per_source_timeout: Duration,
}

impl Aggregator {
    pub fn new(registry: Registry) -> Self {
        Self {
            registry,
            per_source_timeout: Duration::from_secs(9),
        }
    }

    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.per_source_timeout = duration;
        self
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub async fn search(
        &self,
        keyword: &str,
        page: u32,
        limit: u32,
        sources: &[Source],
    ) -> AggregatedPage {
        let keyword = keyword.trim().to_string();
        if keyword.is_empty() {
            return AggregatedPage {
                keyword,
                page,
                tracks: Vec::new(),
                reports: Vec::new(),
            };
        }

        let targets: Vec<Arc<dyn MusicPlatform>> = if sources.is_empty() {
            self.registry.all()
        } else {
            sources
                .iter()
                .filter_map(|source| self.registry.get(*source).ok())
                .collect()
        };

        let limit = limit.clamp(1, 100);
        let timeout_duration = self.per_source_timeout;
        let futures = targets.into_iter().map(|platform| {
            let query = SearchQuery::new(keyword.clone(), page, limit);
            async move {
                let started = Instant::now();
                let result = timeout(timeout_duration, platform.search(&query)).await;
                let elapsed_ms = started.elapsed().as_millis() as u64;
                let source = platform.source();
                match result {
                    Ok(Ok(page)) => Outcome {
                        source,
                        items: page.items,
                        error: None,
                        elapsed_ms,
                    },
                    Ok(Err(err)) => Outcome {
                        source,
                        items: Vec::new(),
                        error: Some(err.to_string()),
                        elapsed_ms,
                    },
                    Err(_) => Outcome {
                        source,
                        items: Vec::new(),
                        error: Some(format!("请求超时（{}s）", timeout_duration.as_secs())),
                        elapsed_ms,
                    },
                }
            }
        });

        let outcomes = join_all(futures).await;
        let reports: Vec<SourceReport> = outcomes
            .iter()
            .map(|outcome| SourceReport {
                source: outcome.source,
                ok: outcome.error.is_none(),
                count: outcome.items.len(),
                elapsed_ms: outcome.elapsed_ms,
                error: outcome.error.clone(),
            })
            .collect();

        let merged = interleave(outcomes.into_iter().map(|o| o.items).collect());
        let mut tracks = group_tracks(merged);
        tracks.sort_by_key(|track| std::cmp::Reverse(relevance(track, &keyword)));
        tracks.truncate(limit as usize);

        AggregatedPage {
            keyword,
            page,
            tracks,
            reports,
        }
    }
}

struct Outcome {
    source: Source,
    items: Vec<Variant>,
    error: Option<String>,
    elapsed_ms: u64,
}

/// 按音源轮转合并，保证首屏结果来自多个音源。
fn interleave(buckets: Vec<Vec<Variant>>) -> Vec<Variant> {
    let mut iters: Vec<std::vec::IntoIter<Variant>> =
        buckets.into_iter().map(|items| items.into_iter()).collect();
    let mut merged = Vec::new();
    loop {
        let mut progressed = false;
        for iter in iters.iter_mut() {
            if let Some(item) = iter.next() {
                merged.push(item);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    merged
}

/// 把同曲记录合并为聚合条目。判重条件：归一化标题一致，且（时长接近 或 歌手有交集）。
fn group_tracks(variants: Vec<Variant>) -> Vec<Track> {
    let mut groups: Vec<Track> = Vec::new();
    for variant in variants {
        let title_key = normalize_title(&variant.title);
        let mut matched = false;
        for track in groups.iter_mut() {
            if normalize_title(&track.title) != title_key {
                continue;
            }
            if same_song(track, &variant) {
                track.absorb(variant.clone());
                matched = true;
                break;
            }
        }
        if !matched {
            groups.push(Track::from_variant(variant));
        }
    }
    groups
}

fn same_song(track: &Track, variant: &Variant) -> bool {
    let duration_ok = track.duration_ms == 0
        || variant.duration_ms == 0
        || track.duration_ms.abs_diff(variant.duration_ms) <= 4_000;
    if !duration_ok {
        return false;
    }
    if track.artists.is_empty() || variant.artists.is_empty() {
        return true;
    }
    track.artists.iter().any(|a| {
        variant
            .artists
            .iter()
            .any(|b| normalize_title(a) == normalize_title(b))
    })
}

/// 相关度打分：标题匹配为主，可用音源数量与音质为辅。
fn relevance(track: &Track, keyword: &str) -> i64 {
    let key = normalize_title(keyword);
    let title = normalize_title(&track.title);
    let mut score = 0i64;
    if title == key {
        score += 1_000;
    } else if title.starts_with(&key) {
        score += 500;
    } else if title.contains(&key) {
        score += 300;
    }
    // 翻唱/改编版（Live、cover、remix、伴奏、各种「版」）在标题归一化后
    // 会和原唱撞名，但原始标题带了这些标记——匿名搜索常把翻唱排前面，
    // 这里把它们压下去，让真正的原唱（标题就是关键词本身）浮上来。
    if is_cover(&track.title) {
        score -= 250;
    }
    if track
        .artists
        .iter()
        .any(|artist| normalize_title(artist).contains(&key))
    {
        score += 200;
    }
    if track.cover.is_some() {
        score += 50;
    }
    if track.fee == crate::model::Fee::Free {
        score += 40;
    }
    score += (track.bitrate.min(999) as i64) / 10;
    score += track.variants.len() as i64 * 30;
    // 时长异常（0 或超过 15 分钟）通常是无用结果，压一档
    if track.duration_ms == 0 || track.duration_ms > 15 * 60 * 1000 {
        score -= 60;
    }
    score
}

/// 判断标题是否带翻唱/改编/现场等标记。
///
/// 网易云匿名搜索常把翻唱（如「晴天(深情版)」「晴天 钢琴版」）排在原唱前面，
/// 通过识别这些后缀词把翻唱降权。
fn is_cover(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    [
        "live",
        "cover",
        "remix",
        "翻唱",
        "翻自",
        "伴奏",
        "纯音乐",
        "钢琴版",
        "吉他版",
        "深情版",
        "女生版",
        "女声版",
        "男生版",
        "男声版",
        "dj版",
        "史诗版",
        "改编",
        "原唱",
        "自唱",
    ]
    .iter()
    .any(|mark| lower.contains(mark))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Fee;

    use crate::model::fingerprint;

    fn variant(source: Source, id: &str, title: &str, artist: &str, duration: u64) -> Variant {
        let mut v = Variant::new(source, id, title);
        v.artists = vec![artist.to_string()];
        v.duration_ms = duration;
        v
    }

    #[test]
    fn merges_same_song_across_sources() {
        let items = vec![
            variant(Source::Netease, "1", "晴天", "周杰伦", 269_000),
            variant(Source::Qq, "2", "晴天", "周杰伦", 270_000),
            variant(Source::Kuwo, "3", "晴天 (Live)", "周杰伦", 271_000),
        ];
        let tracks = group_tracks(items);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].variants.len(), 3);
        assert_eq!(tracks[0].sources().len(), 3);
    }

    #[test]
    fn keeps_different_versions_apart() {
        let items = vec![
            variant(Source::Netease, "1", "晴天", "周杰伦", 269_000),
            variant(Source::Qq, "2", "晴天", "周杰伦", 120_000),
        ];
        let tracks = group_tracks(items);
        assert_eq!(tracks.len(), 2);
    }

    #[test]
    fn interleave_balances_sources() {
        let a = vec![
            variant(Source::Netease, "1", "a", "x", 1000),
            variant(Source::Netease, "2", "b", "x", 1000),
        ];
        let b = vec![variant(Source::Qq, "3", "c", "x", 1000)];
        let merged = interleave(vec![a, b]);
        assert_eq!(
            merged.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "3", "2"]
        );
    }

    #[test]
    fn relevance_prefers_exact_title() {
        let mut exact =
            Track::from_variant(variant(Source::Netease, "1", "晴天", "周杰伦", 269_000));
        exact.fee = Fee::Free;
        let other = Track::from_variant(variant(Source::Qq, "2", "晴天的雨", "周杰伦", 269_000));
        assert!(relevance(&exact, "晴天") > relevance(&other, "晴天"));
        assert!(!fingerprint("晴天", &["周杰伦".into()], 269_000).is_empty());
    }

    #[test]
    fn relevance_demotes_cover_versions() {
        let original =
            Track::from_variant(variant(Source::Netease, "1", "晴天", "周杰伦", 269_000));
        let cover = Track::from_variant(variant(
            Source::Netease,
            "2",
            "晴天(深情版)",
            "某翻唱",
            269_000,
        ));
        // 归一化后两者标题都是「晴天」，但翻唱带标记应被压下去
        assert!(relevance(&original, "晴天") > relevance(&cover, "晴天"));
        assert!(is_cover("晴天 (钢琴版)"));
        assert!(is_cover("晴天 Live"));
        assert!(!is_cover("晴天"));
    }
}
