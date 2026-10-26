use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

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
/// - `decrypt_key` 非空时，写入前对每个 chunk 做 QMCv2 流式解密（`.mflac`/`.mgg`）。
pub async fn download(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    cancel: &AtomicBool,
    pause: &AtomicBool,
    decrypt_key: Option<&str>,
    mut on_progress: impl FnMut(u64, u64),
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
        return Err(Error::Source {
            platform: "transfer".into(),
            reason: format!("下载失败：HTTP {status}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_flags_are_distinct() {
        assert_ne!(TransferOutcome::Completed, TransferOutcome::Paused);
        assert_ne!(TransferOutcome::Paused, TransferOutcome::Canceled);
    }
}
