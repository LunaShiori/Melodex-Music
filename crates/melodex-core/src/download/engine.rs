use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::download::naming::{self, NamingContext, Resolution};
use crate::download::tags::{self, TagMeta};
use crate::download::transfer::{self, TransferOutcome};
use crate::error::{keep_most_actionable, Error, Reason, Result};
use crate::model::{Fee, Source, Variant};
use crate::platform::Registry;
use crate::quality::{self, QualityLevel};
use crate::settings::{default_download_dir, Settings};
use crate::task::{DownloadTask, Fallback, TaskState};

/// 引擎向前端推送的事件。序列化后形如 `{"type":"taskUpsert","payload":{...}}`，
/// 前端按 `type` 分派即可。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "payload")]
pub enum EngineEvent {
    /// 任务快照。装箱是为了压小枚举体积（其余变体只有几十字节）。
    TaskUpsert(Box<DownloadTask>),
    TaskRemoved(String),
    Notice(Notice),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub level: NoticeLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoticeLevel {
    Info,
    Success,
    Warn,
    Error,
}

/// 前端提交下载时传的最小载荷，避免把整个聚合对象塞过 IPC。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueInput {
    pub source: Source,
    pub source_id: String,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub bitrate: Option<u32>,
    /// 目标音质。缺省时按设置里该音源的档位来（分平台设置优先于全局默认）。
    #[serde(default)]
    pub quality: Option<QualityLevel>,
    #[serde(default)]
    pub format: Option<String>,
    /// 同一首歌在其它音源上的记录，首选源取不到链接时依次尝试
    #[serde(default)]
    pub fallbacks: Vec<Fallback>,
}

struct Control {
    pause: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

/// 下载引擎：任务队列 + 调度 + 落盘 + 事件广播。
///
/// 采用「队列 + 泵」模型而不是一次性批量 spawn：每次任务结束都会重新计算
/// 并发额度，因此在设置里改并发数可以立刻生效。
#[derive(Clone)]
pub struct DownloadEngine {
    inner: Arc<Inner>,
}

struct Inner {
    registry: Registry,
    client: reqwest::Client,
    settings: RwLock<Settings>,
    tasks: Mutex<Vec<DownloadTask>>,
    controls: Mutex<HashMap<String, Control>>,
    tx: broadcast::Sender<EngineEvent>,
    store_path: Option<PathBuf>,
    /// 首次在运行时上下文中派发任务时缓存下来的 handle。
    /// 之后即使从没有运行时的线程（Tauri 的同步 command）调用，也能把任务送进同一个运行时。
    rt: OnceLock<tokio::runtime::Handle>,
}

impl DownloadEngine {
    pub fn new(
        registry: Registry,
        settings: Settings,
        store_path: Option<PathBuf>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(crate::net::UA)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()?;
        let (tx, _) = broadcast::channel(256);
        let inner = Arc::new(Inner {
            registry,
            client,
            settings: RwLock::new(settings.sanitized()),
            tasks: Mutex::new(Vec::new()),
            controls: Mutex::new(HashMap::new()),
            tx,
            store_path,
            rt: OnceLock::new(),
        });
        inner.load();
        Ok(Self { inner })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.tx.subscribe()
    }

    pub fn settings(&self) -> Settings {
        self.inner.settings()
    }

    pub fn update_settings(&self, settings: Settings) {
        let sanitized = settings.sanitized();
        *self.inner.settings.write().expect("settings poisoned") = sanitized;
        self.inner.persist();
    }

    pub fn tasks(&self) -> Vec<DownloadTask> {
        self.inner.tasks.lock().expect("tasks poisoned").clone()
    }

    pub fn active_count(&self) -> usize {
        self.inner.controls.lock().expect("controls poisoned").len()
    }

    /// 批量入队，返回任务 ID（顺序与输入一致）。
    pub fn enqueue(&self, inputs: Vec<EnqueueInput>) -> Vec<String> {
        let settings = self.settings();
        let mut ids = Vec::with_capacity(inputs.len());
        {
            let mut tasks = self.inner.tasks.lock().expect("tasks poisoned");
            for input in inputs {
                let quality = input
                    .quality
                    .unwrap_or_else(|| settings.quality_of(input.source));
                let nominal = input.bitrate.unwrap_or_else(|| {
                    quality::find(input.source, quality)
                        .map(|item| item.bitrate)
                        .unwrap_or(128)
                });
                let mut task = DownloadTask::new(
                    input.source,
                    input.source_id.clone(),
                    input.title.clone(),
                    input.artists.clone(),
                    nominal,
                );
                task.quality = quality;
                task.source_id = input.source_id;
                task.album = input.album;
                task.cover = input.cover;
                task.duration_ms = input.duration_ms;
                task.fallbacks = input
                    .fallbacks
                    .into_iter()
                    .filter(|item| item.source != input.source)
                    .collect();
                if let Some(format) = input.format {
                    task.format = format;
                }
                ids.push(task.id.clone());
                tasks.push(task);
            }
        }
        for id in &ids {
            self.inner.emit_upsert(id);
        }
        self.inner.persist();
        pump(self.inner.clone());
        ids
    }

    pub fn pause(&self, id: &str) {
        {
            let mut tasks = self.inner.tasks.lock().expect("tasks poisoned");
            if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
                if task.state == TaskState::Queued {
                    task.mark(TaskState::Paused);
                }
            }
        }
        if let Some(control) = self
            .inner
            .controls
            .lock()
            .expect("controls poisoned")
            .get(id)
        {
            control.pause.store(true, Ordering::Relaxed);
        }
        self.inner.emit_upsert(id);
    }

    pub fn resume(&self, id: &str) {
        {
            let mut tasks = self.inner.tasks.lock().expect("tasks poisoned");
            if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
                if matches!(task.state, TaskState::Paused | TaskState::Failed) {
                    task.error = None;
                    task.mark(TaskState::Queued);
                }
            }
        }
        if let Some(control) = self
            .inner
            .controls
            .lock()
            .expect("controls poisoned")
            .get(id)
        {
            control.pause.store(false, Ordering::Relaxed);
        }
        self.inner.emit_upsert(id);
        self.inner.persist();
        pump(self.inner.clone());
    }

    pub fn cancel(&self, id: &str) {
        if let Some(control) = self
            .inner
            .controls
            .lock()
            .expect("controls poisoned")
            .get(id)
        {
            control.cancel.store(true, Ordering::Relaxed);
        }
        self.inner.mutate(id, |task| {
            if task.state.is_active() && task.state != TaskState::Resolving {
                task.mark(TaskState::Canceled);
            }
        });
    }

    pub fn remove(&self, id: &str) {
        if let Some(control) = self
            .inner
            .controls
            .lock()
            .expect("controls poisoned")
            .get(id)
        {
            control.cancel.store(true, Ordering::Relaxed);
        }
        let removed = {
            let mut tasks = self.inner.tasks.lock().expect("tasks poisoned");
            let before = tasks.len();
            tasks.retain(|task| task.id != id);
            before != tasks.len()
        };
        if removed {
            let _ = self.inner.tx.send(EngineEvent::TaskRemoved(id.to_string()));
            self.inner.persist();
        }
    }

    /// 清理已结束（完成/失败/取消）的任务，返回清理数量。
    pub fn clear_finished(&self) -> usize {
        let removed = {
            let mut tasks = self.inner.tasks.lock().expect("tasks poisoned");
            let before = tasks.len();
            tasks.retain(|task| task.state.is_active() || task.state == TaskState::Paused);
            before - tasks.len()
        };
        if removed > 0 {
            self.inner.persist();
            for task in self.tasks() {
                let _ = self.inner.tx.send(EngineEvent::TaskUpsert(Box::new(task)));
            }
        }
        removed
    }
}

impl Inner {
    fn settings(&self) -> Settings {
        self.settings.read().expect("settings poisoned").clone()
    }

    fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn find(&self, id: &str) -> Option<DownloadTask> {
        self.tasks
            .lock()
            .expect("tasks poisoned")
            .iter()
            .find(|task| task.id == id)
            .cloned()
    }

    fn mutate<F>(&self, id: &str, change: F) -> Option<DownloadTask>
    where
        F: FnOnce(&mut DownloadTask),
    {
        let updated = {
            let mut tasks = self.tasks.lock().expect("tasks poisoned");
            tasks.iter_mut().find(|task| task.id == id).map(|task| {
                change(task);
                task.clone()
            })
        };
        if let Some(task) = updated.clone() {
            let _ = self.tx.send(EngineEvent::TaskUpsert(Box::new(task)));
        }
        updated
    }

    fn emit_upsert(&self, id: &str) {
        if let Some(task) = self.find(id) {
            let _ = self.tx.send(EngineEvent::TaskUpsert(Box::new(task)));
        }
    }

    fn notice(&self, level: NoticeLevel, message: impl Into<String>) {
        let _ = self.tx.send(EngineEvent::Notice(Notice {
            level,
            message: message.into(),
        }));
    }

    fn persist(&self) {
        let Some(path) = self.store_path.as_ref() else {
            return;
        };
        let tasks = self.tasks.lock().expect("tasks poisoned").clone();
        let payload = PersistedTasks { tasks };
        if let Ok(json) = serde_json::to_string_pretty(&payload) {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, json);
        }
    }

    fn load(&self) {
        let Some(path) = self.store_path.as_ref() else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(mut payload) = serde_json::from_str::<PersistedTasks>(&text) else {
            return;
        };
        // 上次退出时还在跑的任务一律降级为「已暂停」，等用户手动继续，
        // 避免启动瞬间并发抢占带宽。
        for task in payload.tasks.iter_mut() {
            if task.state.is_active() {
                task.state = TaskState::Paused;
                task.speed_bps = 0;
            }
        }
        *self.tasks.lock().expect("tasks poisoned") = payload.tasks;
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedTasks {
    tasks: Vec<DownloadTask>,
}

fn pump(inner: Arc<Inner>) {
    let limit = inner.settings().concurrency.max(1);
    loop {
        let running = inner.controls.lock().expect("controls poisoned").len();
        if running >= limit {
            break;
        }
        let next_id = {
            let mut tasks = inner.tasks.lock().expect("tasks poisoned");
            match tasks
                .iter_mut()
                .find(|task| task.state == TaskState::Queued)
            {
                Some(task) => {
                    task.mark(TaskState::Resolving);
                    Some(task.id.clone())
                }
                None => None,
            }
        };
        let Some(id) = next_id else { break };

        let pause = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        inner.controls.lock().expect("controls poisoned").insert(
            id.clone(),
            Control {
                pause: pause.clone(),
                cancel: cancel.clone(),
            },
        );
        inner.emit_upsert(&id);

        let task_inner = inner.clone();
        spawn_background(&inner.rt, async move {
            run_task(task_inner, id, pause, cancel).await;
        });
    }
}

/// 把下载任务派发到 Tokio 运行时上执行。
///
/// 这里必须兜底，不能直接写 `tokio::spawn`：Tauri 的**同步** command 运行在
/// 主线程上，那里没有 Tokio 运行时上下文，`tokio::spawn` 会 panic
/// （`there is no reactor running, must be called from the context of a Tokio 1.x runtime`），
/// 而 panic 会穿过 WebView 的 IPC 回调，直接把整个进程 abort —— 表现为"点下载就闪退"。
///
/// 三级策略：
/// 1. 当前线程已在运行时上下文内（异步 command、或已 spawn 的任务里）→ 直接用它，并缓存 handle；
/// 2. 用之前缓存的 handle（覆盖同步 command 等无上下文的调用点）；
/// 3. 极端情况下都没有，就开一个独立线程建单线程运行时执行，保证任务不被静默丢弃。
fn spawn_background(
    cached: &OnceLock<tokio::runtime::Handle>,
    future: impl Future<Output = ()> + Send + 'static,
) {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let _ = cached.set(handle.clone());
        handle.spawn(future);
        return;
    }
    if let Some(handle) = cached.get() {
        handle.spawn(future);
        return;
    }
    std::thread::spawn(move || {
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt.block_on(future),
            Err(err) => eprintln!("Melodex: 无法创建运行时，下载任务已放弃：{err}"),
        }
    });
}

enum Step {
    Completed(String),
    Paused,
    Canceled,
    Skipped(String),
}

async fn run_task(inner: Arc<Inner>, id: String, pause: Arc<AtomicBool>, cancel: Arc<AtomicBool>) {
    let result = execute(inner.clone(), &id, &pause, &cancel).await;
    inner
        .controls
        .lock()
        .expect("controls poisoned")
        .remove(&id);

    let title = inner
        .find(&id)
        .map(|task| task.title)
        .unwrap_or_else(|| id.clone());

    match result {
        Ok(Step::Completed(path)) => {
            inner.mutate(&id, |task| {
                task.path = Some(path);
                task.error = None;
                task.mark(TaskState::Completed);
            });
            inner.notice(NoticeLevel::Success, format!("《{title}》下载完成"));
        }
        Ok(Step::Skipped(path)) => {
            inner.mutate(&id, |task| {
                task.path = Some(path);
                task.mark(TaskState::Completed);
            });
            inner.notice(NoticeLevel::Info, format!("《{title}》已存在，按策略跳过"));
        }
        Ok(Step::Paused) => {
            inner.mutate(&id, |task| task.mark(TaskState::Paused));
            inner.notice(NoticeLevel::Warn, format!("《{title}》已暂停"));
        }
        Ok(Step::Canceled) => {
            inner.mutate(&id, |task| task.mark(TaskState::Canceled));
        }
        Err(err) => {
            inner.mutate(&id, |task| {
                task.fail_reason = err.reason();
                task.error = Some(err.to_string());
                task.mark(TaskState::Failed);
            });
            inner.notice(NoticeLevel::Error, format!("《{title}》下载失败：{err}"));
        }
    }
    inner.persist();
    pump(inner);
}

async fn execute(
    inner: Arc<Inner>,
    id: &str,
    pause: &AtomicBool,
    cancel: &AtomicBool,
) -> Result<Step> {
    let task = inner
        .find(id)
        .ok_or_else(|| Error::TaskNotFound(id.into()))?;
    let settings = inner.settings();

    // 1) 取链接：首选音源 → 其它音源，每个源内部再按「目标档位 → 逐档降级」。
    //
    // 同一首歌在不同音源的可获取性差异极大（QQ 要会员、酷我免费是常态），
    // 所以首选源失败时不该直接判死，而是按用户当初的排序换源再试。
    // 换源沿用同一个「等级」，具体落到哪一档由各平台自己的档位表截断。
    let mut candidates: Vec<(Source, String)> = vec![(task.source, task.source_id.clone())];
    candidates.extend(
        task.fallbacks
            .iter()
            .map(|item| (item.source, item.source_id.clone())),
    );

    let mut play: Option<crate::model::PlayUrl> = None;
    let mut resolved: Option<(Source, String)> = None;
    // 记录信息量最大的失败原因（NeedLogin 比 QualityMissing 有用得多）
    let mut best: Option<Error> = None;

    'candidates: for (source, source_id) in candidates {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Step::Canceled);
        }
        let platform = match inner.registry.get(source) {
            Ok(platform) => platform,
            Err(err) => {
                keep_most_actionable(&mut best, err);
                continue;
            }
        };
        let mut ladder = quality::ladder(source, task.quality);
        // 关掉自动降级时只试用户选中的那一档：宁可失败，也不要偷偷给个低音质
        if !settings.auto_fallback && ladder.len() > 1 {
            ladder.truncate(1);
        }
        let variant = Variant {
            source,
            id: source_id.clone(),
            title: task.title.clone(),
            artists: task.artists.clone(),
            album: task.album.clone(),
            duration_ms: task.duration_ms,
            cover: task.cover.clone(),
            bitrate: ladder
                .first()
                .map(|item| item.bitrate)
                .unwrap_or(task.bitrate),
            format: task.format.clone(),
            size: 0,
            fee: Fee::Unknown,
            // 下载任务只带 songmid，不带 media_mid：QQ 的 resolve 发现它是 None
            // 会自己去歌曲详情里补真实值，比把搜索结果的 mid 一路传下来更可靠
            // （歌单接口干脆不给 media_mid）。
            media_mid: None,
        };
        for item in ladder {
            if cancel.load(Ordering::Relaxed) {
                return Ok(Step::Canceled);
            }
            match platform.resolve(&variant, item).await {
                Ok(result) => {
                    resolved = Some((source, source_id));
                    play = Some(result);
                    break 'candidates;
                }
                Err(err) => keep_most_actionable(&mut best, err),
            }
        }
    }

    let play = match play {
        Some(play) => play,
        None => {
            return Err(best.unwrap_or_else(|| Error::unavailable(task.source, Reason::Unknown)))
        }
    };

    let extension = normalize_ext(&play.format);
    // 后面取歌词要用「实际拿到链接的那个音源」，换过源时不能再用首选源的 ID
    let (play_source, play_id) = resolved
        .clone()
        .unwrap_or_else(|| (task.source, task.source_id.clone()));
    let platform = inner.registry.get(play_source)?;
    let switched = resolved.as_ref().map(|(source, _)| *source != task.source);
    inner.mutate(id, |task| {
        task.format = extension.clone();
        task.bitrate = play.bitrate;
        task.resolved_source = resolved.as_ref().map(|(source, _)| *source);
        task.error = None;
        task.fail_reason = None;
    });
    if let Some(true) = switched {
        if let Some((source, _)) = resolved.as_ref() {
            inner.notice(
                NoticeLevel::Info,
                format!(
                    "《{}》原音源不可用，已自动改用{}",
                    task.title,
                    source.label()
                ),
            );
        }
    }

    // 2) 目标路径与重复策略
    let dir = if settings.download_dir.trim().is_empty() {
        default_download_dir()
    } else {
        PathBuf::from(settings.download_dir.trim())
    };
    tokio::fs::create_dir_all(&dir).await?;
    let stem = naming::render(
        &settings.naming_template,
        &NamingContext {
            title: &task.title,
            artists: &task.artists,
            album: &task.album,
            source: task.source.id(),
            bitrate: play.bitrate,
            index: 0,
            artist_separator: &settings.artist_separator,
        },
    );
    let target = match naming::resolve_target(&dir, &stem, &extension, settings.duplicate_policy) {
        Resolution::Skip(path) => return Ok(Step::Skipped(path.to_string_lossy().into_owned())),
        Resolution::Write(path) => path,
    };
    let part = dir.join(format!("{stem}.{extension}.part"));

    // 3) 下载
    inner.mutate(id, |task| {
        task.mark(TaskState::Downloading);
        task.total = play.size;
        task.downloaded = 0;
    });

    let mut last_emit = Instant::now();
    let mut last_bytes = 0u64;
    let mut last_at = Instant::now();
    let progress_inner = inner.clone();
    let progress_id = id.to_string();
    let outcome = transfer::download(
        inner.client(),
        &play.url,
        &part,
        cancel,
        pause,
        play.ekey.as_deref(),
        move |downloaded, total| {
            let now = Instant::now();
            if now.duration_since(last_emit) < Duration::from_millis(240) {
                return;
            }
            let elapsed = now.duration_since(last_at).as_secs_f64();
            let speed = if elapsed > 0.2 {
                ((downloaded.saturating_sub(last_bytes)) as f64 / elapsed) as u64
            } else {
                0
            };
            last_bytes = downloaded;
            last_at = now;
            last_emit = now;
            progress_inner.mutate(&progress_id, |task| {
                task.downloaded = downloaded;
                if total > 0 {
                    task.total = total;
                }
                task.speed_bps = speed;
            });
        },
    )
    .await?;

    match outcome.outcome {
        TransferOutcome::Paused => return Ok(Step::Paused),
        TransferOutcome::Canceled => {
            let _ = tokio::fs::remove_file(&part).await;
            return Ok(Step::Canceled);
        }
        TransferOutcome::Completed => {}
    }

    inner.mutate(id, |task| {
        task.downloaded = outcome.downloaded;
        if outcome.total > 0 {
            task.total = outcome.total;
        }
    });

    tokio::fs::rename(&part, &target).await?;

    // 4) 标签与歌词
    inner.mutate(id, |task| task.mark(TaskState::Tagging));
    let lyric = if settings.write_tags || settings.save_lyric {
        platform
            .lyric(&play_id)
            .await
            .ok()
            .flatten()
            .map(|lyric| lyric.lrc)
    } else {
        None
    };

    if settings.save_lyric {
        if let Some(ref lrc) = lyric {
            if !lrc.trim().is_empty() {
                let lrc_path = target.with_extension("lrc");
                let _ = tokio::fs::write(&lrc_path, lrc).await;
            }
        }
    }

    if settings.write_tags && extension == "mp3" {
        let cover = if settings.embed_cover {
            fetch_cover(inner.client(), task.cover.as_deref()).await
        } else {
            None
        };
        let meta = TagMeta {
            title: task.title.clone(),
            artists: task.artists.join(&settings.artist_separator),
            album: task.album.clone(),
            source: task.source.label().to_string(),
            cover,
            cover_mime: "image/jpeg".into(),
            lyric,
        };
        let path = target.clone();
        // 标签写入是纯同步 IO，丢到阻塞线程池避免卡住 runtime
        let _ = tokio::task::spawn_blocking(move || tags::write_mp3_tags(&path, &meta)).await;
    }

    Ok(Step::Completed(target.to_string_lossy().into_owned()))
}

async fn fetch_cover(client: &reqwest::Client, url: Option<&str>) -> Option<Vec<u8>> {
    let url = url?;
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    if let Some(len) = response.content_length() {
        // 超过 4MB 的封面没有嵌入意义，直接放弃
        if len > 4 * 1024 * 1024 {
            return None;
        }
    }
    response.bytes().await.ok().map(|bytes| bytes.to_vec())
}

fn normalize_ext(format: &str) -> String {
    let lowered = format.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "mp3" | "flac" | "m4a" | "aac" | "ogg" | "wav" => lowered,
        "mp4" => "m4a".to_string(),
        // 加密格式下载后会被 QMCv2 解密成明文，扩展名也随之还原
        "mflac" => "flac".to_string(),
        "mgg" => "ogg".to_string(),
        _ => "mp3".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_extensions() {
        assert_eq!(normalize_ext("MP3"), "mp3");
        assert_eq!(normalize_ext("mp4"), "m4a");
        assert_eq!(normalize_ext("weird"), "mp3");
    }

    #[test]
    fn enqueue_input_parses_camel_case() {
        let input: EnqueueInput = serde_json::from_str(
            r#"{"source":"qq","sourceId":"0039MnYb0qxYhV","title":"晴天","artists":["周杰伦"],"bitrate":320}"#,
        )
        .unwrap();
        assert_eq!(input.source, Source::Qq);
        assert_eq!(input.source_id, "0039MnYb0qxYhV");
        assert_eq!(input.bitrate, Some(320));
        assert!(input.fallbacks.is_empty());
    }

    #[test]
    fn enqueue_input_parses_fallbacks() {
        let input: EnqueueInput = serde_json::from_str(
            r#"{"source":"qq","sourceId":"a","title":"x",
                "fallbacks":[{"source":"kuwo","sourceId":"MUSIC_1","bitrate":320}]}"#,
        )
        .unwrap();
        assert_eq!(input.fallbacks.len(), 1);
        assert_eq!(input.fallbacks[0].source, Source::Kuwo);
    }

    /// 换源失败时不能把「无此音质」当成最终原因盖掉「需要登录」。
    #[test]
    fn keeps_the_most_actionable_reason() {
        let mut best = None;
        keep_most_actionable(
            &mut best,
            Error::unavailable(Source::Qq, Reason::QualityMissing),
        );
        keep_most_actionable(&mut best, Error::unavailable(Source::Qq, Reason::NeedLogin));
        keep_most_actionable(
            &mut best,
            Error::unavailable(Source::Kuwo, Reason::Copyright),
        );
        assert_eq!(
            best.as_ref().and_then(Error::reason),
            Some(Reason::NeedLogin)
        );
    }

    #[test]
    fn keeps_first_error_when_ranks_tie() {
        let mut best = None;
        keep_most_actionable(&mut best, Error::unavailable(Source::Qq, Reason::NeedLogin));
        keep_most_actionable(
            &mut best,
            Error::unavailable(Source::Kuwo, Reason::NeedLogin),
        );
        let message = best.unwrap().to_string();
        assert!(message.contains("QQ 音乐"), "{message}");
    }

    /// 前端靠 `type`/`payload` 分派事件，装箱不能改变 JSON 形状。
    #[test]
    fn engine_event_keeps_json_shape() {
        let task = DownloadTask::new(
            Source::Netease,
            "186016",
            "晴天",
            vec!["周杰伦".into()],
            320,
        );
        let value = serde_json::to_value(EngineEvent::TaskUpsert(Box::new(task))).unwrap();

        assert_eq!(value["type"], "taskUpsert");
        assert_eq!(value["payload"]["title"], "晴天");
        assert_eq!(value["payload"]["source"], "netease");
        assert_eq!(value["payload"]["state"], "queued");

        let removed = serde_json::to_value(EngineEvent::TaskRemoved("abc".into())).unwrap();
        assert_eq!(removed["type"], "taskRemoved");
        assert_eq!(removed["payload"], "abc");
    }

    /// 回归测试：在**没有** Tokio 运行时上下文的线程上派发任务不能 panic。
    ///
    /// 这正是 0.1.0 崩溃的成因——Tauri 同步 command 在主线程调用 `pump()`，
    /// 裸 `tokio::spawn` 直接 panic 并 abort 进程。测试线程同样没有运行时，
    /// 因此能稳定复现该场景。
    #[test]
    fn spawning_outside_runtime_does_not_panic() {
        let cached: OnceLock<tokio::runtime::Handle> = OnceLock::new();
        let ran = Arc::new(AtomicBool::new(false));

        let flag = ran.clone();
        spawn_background(&cached, async move {
            flag.store(true, Ordering::SeqCst);
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !ran.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ran.load(Ordering::SeqCst), "兜底线程没有把任务跑起来");
    }
}
