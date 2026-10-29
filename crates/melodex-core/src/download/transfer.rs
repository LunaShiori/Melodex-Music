use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOutcome {
    Completed,
    Paused,
    Canceled,
}

#[derive(Debug, Clone, Copy)]
pub struct TransferResult {
    pub outcome: TransferOutcome,
    pub downloaded: u64,
    pub total: u64,
}

/// 带断点续传的流式下载。
///
/// - 先写 `.part` 临时文件，成功后再改名，避免半成品被当成成品；
/// - 已有 `.part` 时用 `Range` 续传，若服务端不支持（返回 200）则自动从头写；
/// - 每个 chunk 检查一次取消/暂停标志，保证响应及时；
/// - `decrypt_key` 非空时，写入前对每个 chunk 做 QMCv2 流式解密（`.mflac`/`.mgg`）；
/// - `speed_limit_bps` 非 0 时按单任务限速，每个 chunk 后补 sleep（见 [`throttle_delay`]）。
// 参数都是下载一股脑要的东西，硬拆成结构体反而每次调用都要多写一行；8 个参数在这个位置是可读的。
#[allow(clippy::too_many_arguments)]
pub async fn download(
    client: &reqwest::Client,
    url: &str,
    // 音源要求的请求头（Referer / Origin 等）：QQ 的 CDN 不带 Referer 直接 403
    headers: &[(&'static str, &'static str)],
    part: &Path,
    cancel: &AtomicBool,
    pause: &AtomicBool,
    decrypt_key: Option<&str>,
    speed_limit_bps: u64,
    on_progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<TransferResult> {
    let existing = tokio::fs::metadata(part)
        .await
        .map(|meta| meta.len())
        .unwrap_or(0);

    // 解密器按绝对字节偏移解密，续传时从已写入的长度继续
    let cipher = match decrypt_key.filter(|key| !key.is_empty()) {
        Some(key) => umc_qmc::QMCv2Cipher::new_from_ekey(key.as_bytes()).ok(),
        None => None,
    };

    let mut request = client.get(url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    if existing > 0 {
        request = request.header("Range", format!("bytes={existing}-"));
    }
    let response = request.send().await?;
    let status = response.status();
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        // 临时文件比远端还大，说明上次写入异常，删掉重来
        let _ = tokio::fs::remove_file(part).await;
        return Err(Error::Source {
            platform: "transfer".into(),
            reason: "断点位置越界，已清理临时文件，请重试".into(),
        });
    }
    if !status.is_success() {
        // 交给引擎判断：403/404 多半是链接过期或绑定的 IP 变了，值得重取一次。
        return Err(Error::HttpStatus {
            status: status.as_u16(),
        });
    }

    let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
    let body_len = response.content_length().unwrap_or(0);
    let total = if resumed {
        existing + body_len
    } else {
        body_len
    };
    let mut downloaded = if resumed { existing } else { 0 };

    let mut file = if resumed {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(part)
            .await?
    } else {
        tokio::fs::File::create(part).await?
    };

    let mut stream = response.bytes_stream();
    // 限速只算「本次传输」的字节：续传时 .part 里已有的那部分不算进配额，
    // 否则一续传就会被判定成严重超速，白白等半天。
    let rate_base = downloaded;
    let rate_started = Instant::now();
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            file.flush().await?;
            return Ok(TransferResult {
                outcome: TransferOutcome::Canceled,
                downloaded,
                total,
            });
        }
        if pause.load(Ordering::Relaxed) {
            file.flush().await?;
            return Ok(TransferResult {
                outcome: TransferOutcome::Paused,
                downloaded,
                total,
            });
        }
        let bytes = chunk?;
        // 解密在写入前进行，偏移是当前已下载的绝对位置。
        // QMCv2 是流式的，chunk 边界任意都不影响结果，`decrypt` 内部会按段切分。
        // `bytes::Bytes` 是共享不可变的，解密需要可变切片，所以先拷一份。
        let chunk = match cipher {
            Some(ref cipher) => {
                let mut buf = bytes.to_vec();
                cipher.decrypt(&mut buf[..], downloaded as usize);
                buf
            }
            None => bytes.to_vec(),
        };
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);

        // 限速在写盘之后补睡：先让进度条动起来，再按配额压住下一段。
        if let Some(delay) = throttle_delay(
            downloaded - rate_base,
            speed_limit_bps,
            rate_started.elapsed(),
        ) {
            tokio::time::sleep(delay).await;
        }
    }
    file.flush().await?;
    drop(file);

    if total > 0 && downloaded < total {
        return Err(Error::Source {
            platform: "transfer".into(),
            reason: format!("数据不完整：{downloaded}/{total}"),
        });
    }

    Ok(TransferResult {
        outcome: TransferOutcome::Completed,
        downloaded,
        total,
    })
}

/// 单任务限速：算「按目标速率还欠多少时间」。
///
/// - `limit_bps == 0`：不限速，永远不 sleep；
/// - 已经落后于配额（网速本来就慢）：返回 `None`，不额外拖慢；
/// - 单次最多睡 1 秒：暂停/取消最多晚 1 秒被处理；chunk 均匀时，
///   欠的账会累积到下一轮，平均速率仍收敛到设定值。
fn throttle_delay(downloaded: u64, limit_bps: u64, elapsed: Duration) -> Option<Duration> {
    if limit_bps == 0 {
        return None;
    }
    let target = Duration::from_secs_f64(downloaded as f64 / limit_bps as f64);
    let gap = target.checked_sub(elapsed)?;
    if gap.is_zero() {
        None
    } else {
        Some(gap.min(Duration::from_secs(1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_flags_are_distinct() {
        assert_ne!(TransferOutcome::Completed, TransferOutcome::Paused);
        assert_ne!(TransferOutcome::Paused, TransferOutcome::Canceled);
    }

    #[test]
    fn throttle_is_noop_when_unlimited() {
        assert_eq!(throttle_delay(10_000, 0, Duration::from_millis(10)), None);
    }

    #[test]
    fn throttle_waits_for_the_quota() {
        // 1000 字节 / 1000 B/s = 目标 1s，已经过了 0.4s → 还差 0.6s
        let delay = throttle_delay(1_000, 1_000, Duration::from_millis(400)).unwrap();
        assert!((delay.as_millis() as i64 - 600).abs() <= 2, "got {delay:?}");
    }

    #[test]
    fn throttle_does_not_slow_a_slow_connection() {
        // 只下了 100 字节却已经过了 2s，比配额还慢，不该再等
        assert_eq!(throttle_delay(100, 1_000, Duration::from_secs(2)), None);
    }

    #[test]
    fn throttle_caps_one_sleep() {
        // 100 字节 @ 10 B/s → 目标 10s，单次最多睡 1s
        let delay = throttle_delay(100, 10, Duration::ZERO).unwrap();
        assert_eq!(delay, Duration::from_secs(1));
    }
}
