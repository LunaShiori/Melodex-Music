use serde::{Deserialize, Serialize};

use crate::error::Reason;
use crate::model::Source;

/// 同一首歌在**其它**音源上的候选记录。
///
/// 一首歌在 QQ 音乐要会员、在酷我免费是常态。入队时把其它音源的 ID 一起带上，
/// 首选音源拿不到链接时引擎可以自动换源，而不是直接判失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fallback {
    pub source: Source,
    pub source_id: String,
    /// 该音源上这首的标称音质，仅用于展示
    #[serde(default)]
    pub bitrate: u32,
}

/// 下载任务状态机：
/// `Queued → Resolving → Downloading → Tagging → Completed`，
/// 任意阶段可进入 `Paused / Canceled / Failed`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Queued,
    Resolving,
    Downloading,
    Tagging,
    Completed,
    Failed,
    Paused,
    Canceled,
}

impl TaskState {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            TaskState::Queued | TaskState::Resolving | TaskState::Downloading | TaskState::Tagging
        )
    }

    pub fn is_finished(self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Canceled
        )
    }
}

/// 一条下载任务。字段同时供前端渲染与持久化恢复使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub cover: Option<String>,
    pub source: Source,
    pub source_id: String,
    pub bitrate: u32,
    pub format: String,
    pub duration_ms: u64,
    pub state: TaskState,
    pub downloaded: u64,
    pub total: u64,
    pub speed_bps: u64,
    pub path: Option<String>,
    pub error: Option<String>,
    /// 失败的结构化原因，界面据此决定显示「去登录」还是「重试」
    #[serde(default)]
    pub fail_reason: Option<Reason>,
    /// 其它音源上的候选，首选源失败时自动换源
    #[serde(default)]
    pub fallbacks: Vec<Fallback>,
    /// 最终真正取到链接的音源（可能与 `source` 不同）
    #[serde(default)]
    pub resolved_source: Option<Source>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

impl DownloadTask {
    pub fn new(
        source: Source,
        source_id: impl Into<String>,
        title: impl Into<String>,
        artists: Vec<String>,
        bitrate: u32,
    ) -> Self {
        let id = make_id(&format!("{}:{}", source.id(), source_id.into()));
        Self {
            id,
            title: title.into(),
            artists,
            album: String::new(),
            cover: None,
            source,
            source_id: String::new(),
            bitrate,
            format: "mp3".into(),
            duration_ms: 0,
            state: TaskState::Queued,
            downloaded: 0,
            total: 0,
            speed_bps: 0,
            path: None,
            error: None,
            fail_reason: None,
            fallbacks: Vec::new(),
            resolved_source: None,
            created_at: now_secs(),
            finished_at: None,
        }
    }

    pub fn artist_line(&self) -> String {
        self.artists.join(" / ")
    }

    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
    }

    pub fn mark(&mut self, state: TaskState) {
        self.state = state;
        if state.is_finished() {
            self.finished_at = Some(now_secs());
        }
        if state != TaskState::Downloading {
            self.speed_bps = 0;
        }
    }
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 由「音源 + 音源内 ID + 时间戳」生成任务 ID，保证同一首歌重复加入时仍可区分。
pub fn make_id(seed: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in seed.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:x}{:04x}", nanos, (hash & 0xffff) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_bounded() {
        let mut task = DownloadTask::new(Source::Qq, "abc", "晴天", vec!["周杰伦".into()], 320);
        assert_eq!(task.progress(), 0.0);
        task.downloaded = 50;
        task.total = 100;
        assert!((task.progress() - 0.5).abs() < f64::EPSILON);
        task.downloaded = 200;
        assert_eq!(task.progress(), 1.0);
    }

    #[test]
    fn marking_finished_sets_timestamp() {
        let mut task = DownloadTask::new(Source::Netease, "1", "x", vec![], 128);
        assert!(task.finished_at.is_none());
        task.mark(TaskState::Completed);
        assert!(task.finished_at.is_some());
        assert!(task.state.is_finished());
    }

    #[test]
    fn ids_are_unique_enough() {
        let a = make_id("qq:1");
        let b = make_id("qq:1");
        assert_ne!(a, b);
        assert!(a.len() >= 8);
    }

    /// 旧版本落盘的任务 JSON 没有 failReason / fallbacks 字段，反序列化不能失败，
    /// 否则升级后历史任务全部丢失。
    #[test]
    fn old_persisted_task_without_new_fields_still_parses() {
        let json = r#"{
            "id":"abc","title":"晴天","artists":["周杰伦"],"album":"","cover":null,
            "source":"qq","sourceId":"0039MnYb0qxYhV","bitrate":320,"format":"mp3",
            "durationMs":269000,"state":"completed","downloaded":0,"total":0,
            "speedBps":0,"path":null,"error":null,"createdAt":1,"finishedAt":2
        }"#;
        let task: DownloadTask = serde_json::from_str(json).unwrap();
        assert!(task.fail_reason.is_none());
        assert!(task.fallbacks.is_empty());
        assert!(task.resolved_source.is_none());
    }

    #[test]
    fn fallback_round_trips() {
        let mut task = DownloadTask::new(Source::Qq, "1", "x", vec![], 320);
        task.fallbacks.push(Fallback {
            source: Source::Kuwo,
            source_id: "MUSIC_1".into(),
            bitrate: 320,
        });
        task.fail_reason = Some(Reason::NeedLogin);
        let json = serde_json::to_string(&task).unwrap();
        let back: DownloadTask = serde_json::from_str(&json).unwrap();
        assert_eq!(back.fail_reason, Some(Reason::NeedLogin));
        assert_eq!(back.fallbacks.len(), 1);
        assert_eq!(back.fallbacks[0].source, Source::Kuwo);
    }
}
