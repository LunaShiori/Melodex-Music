//! 联调探针：直接跑通「搜索 → 取链接 → 下载 → 打标签」全链路。
//!
//! ```bash
//! cargo run --example probe -- 晴天
//! cargo run --example probe -- 晴天 --download
//! ```

use std::path::PathBuf;

use melodex_core::download::engine::EnqueueInput;
use melodex_core::download::DownloadEngine;
use melodex_core::lyric;
use melodex_core::{build_aggregator, CredStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let keyword = args.next().unwrap_or_else(|| "晴天".to_string());
    let download = args.any(|arg| arg == "--download");

    let creds = CredStore::new();
    let aggregator = build_aggregator(creds.clone())?;

    println!("搜索：{keyword}");
    let page = aggregator.search(&keyword, 1, 20, &[]).await;
    for report in &page.reports {
        println!(
            "  [{}] {} 条 {}ms {}",
            report.source.id(),
            report.count,
            report.elapsed_ms,
            report.error.clone().unwrap_or_default()
        );
    }
    println!("聚合后 {} 条结果", page.tracks.len());

    for (index, track) in page.tracks.iter().take(5).enumerate() {
        println!(
            "{}. {} - {} | {} | {}kbps | 可用音源：{:?}",
            index + 1,
            track.title,
            track.artist_line(),
            if track.album.is_empty() {
                "未知专辑"
            } else {
                &track.album
            },
            track.bitrate,
            track.sources().iter().map(|s| s.id()).collect::<Vec<_>>()
        );
    }

    let Some(best) = page.tracks.first() else {
        println!("没有结果");
        return Ok(());
    };
    let variant = best.best_variant(None).ok_or("no variant")?.clone();
    println!(
        "\n优先尝试：{} / {} / {}",
        variant.source.label(),
        variant.title,
        variant.bitrate
    );

    let platform = aggregator.registry().get(variant.source)?;
    // 探测用「无损」这一档，平台没有无损时 find 会自动落到它的上限
    let quality = melodex_core::quality::find(
        variant.source,
        melodex_core::quality::QualityLevel::Lossless,
    )
    .ok_or("这个音源没有可用档位")?;
    match platform.resolve(&variant, quality).await {
        Ok(play) => println!(
            "取链成功：{} ({}, {}kbps, {} bytes)",
            &play.url[..play.url.len().min(90)],
            play.format,
            play.bitrate,
            play.size
        ),
        Err(err) => println!("取链失败：{err}"),
    }

    if let Ok(Some(text)) = platform.lyric(&variant.id).await {
        let lines = lyric::parse_lrc(&text.lrc);
        println!(
            "歌词 {} 行，首行：{}",
            lines.len(),
            lines.first().map(|l| l.text.as_str()).unwrap_or("")
        );
    }

    if download {
        let registry = melodex_core::Registry::new(melodex_core::net::Net::new()?, creds);
        let dir = PathBuf::from("melodex-probe-output");
        let settings = melodex_core::Settings {
            download_dir: dir.to_string_lossy().to_string(),
            ..Default::default()
        };
        let engine = DownloadEngine::new(registry, settings, None)?;
        let mut events = engine.subscribe();
        let ids = engine.enqueue(vec![EnqueueInput {
            source: variant.source,
            source_id: variant.id.clone(),
            title: variant.title.clone(),
            artists: variant.artists.clone(),
            album: variant.album.clone(),
            cover: variant.cover.clone(),
            duration_ms: variant.duration_ms,
            bitrate: Some(320),
            // 不指定档位时按设置里的目标音质走
            quality: None,
            format: Some(variant.format.clone()),
            fallbacks: Vec::new(),
        }]);
        println!("已入队：{:?}", ids);

        let watcher = tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                match event {
                    melodex_core::download::EngineEvent::TaskUpsert(task) => println!(
                        "  {:?} {:.0}% {} B/s",
                        task.state,
                        task.progress() * 100.0,
                        task.speed_bps
                    ),
                    melodex_core::download::EngineEvent::Notice(notice) => {
                        println!("  * {}", notice.message)
                    }
                    _ => {}
                }
            }
        });

        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        watcher.abort();
        println!("演示目录：{}", dir.display());
    }

    Ok(())
}
