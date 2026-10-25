//! Tauri 外壳：把 Rust 核心的能力暴露成 command，并转发引擎事件给前端。
//!
//! 这一层刻意保持"薄"：不做业务判断，只做参数转换与错误字符串化，
//! 所有逻辑都在 `melodex-core` 里，便于单独测试。

use std::path::PathBuf;

use melodex_core::{
    AggregatedPage, Aggregator, CredStore, DownloadEngine, EnqueueInput, Lyric, PlayUrl,
    PlaylistDetail, Registry, Settings, Source, Variant,
};
use tauri::{Emitter, Manager, State};

pub struct AppState {
    aggregator: Aggregator,
    registry: Registry,
    engine: DownloadEngine,
    /// 更新检查复用同一个 HTTP 客户端（连接池共享，避免额外 TLS 握手）
    net: melodex_core::net::Net,
    settings_path: PathBuf,
}

impl AppState {
    fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|err| format!("无法定位应用数据目录：{err}"))?;
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;

        let settings_path = dir.join("settings.json");
        let settings = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            .unwrap_or_default();

        let net = melodex_core::net::Net::new().map_err(|err| err.to_string())?;
        let creds = CredStore::new();
        let registry = Registry::new(net.clone(), creds);
        let engine = DownloadEngine::new(
            registry.clone(),
            settings,
            Some(dir.join("tasks.json")),
        )
        .map_err(|err| err.to_string())?;
        let aggregator = Aggregator::new(registry.clone());

        Ok(Self {
            aggregator,
            registry,
            engine,
            net,
            settings_path,
        })
    }
}

fn parse_source(raw: &str) -> Result<Source, String> {
    Source::from_id(raw).ok_or_else(|| format!("未知音源：{raw}"))
}

#[tauri::command]
async fn search(
    state: State<'_, AppState>,
    keyword: String,
    page: u32,
    limit: u32,
    sources: Vec<String>,
) -> Result<AggregatedPage, String> {
    let targets: Vec<Source> = sources.iter().filter_map(|s| Source::from_id(s)).collect();
    Ok(state.aggregator.search(&keyword, page, limit, &targets).await)
}

#[tauri::command]
async fn resolve(
    state: State<'_, AppState>,
    source: String,
    id: String,
    bitrate: u32,
) -> Result<PlayUrl, String> {
    let source = parse_source(&source)?;
    let platform = state.registry.get(source).map_err(|err| err.to_string())?;
    let variant = Variant::new(source, id, String::new());
    platform
        .resolve(&variant, bitrate)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn lyric(
    state: State<'_, AppState>,
    source: String,
    id: String,
) -> Result<Option<Lyric>, String> {
    let source = parse_source(&source)?;
    let platform = state.registry.get(source).map_err(|err| err.to_string())?;
    platform.lyric(&id).await.map_err(|err| err.to_string())
}

#[tauri::command]
async fn playlist(
    state: State<'_, AppState>,
    source: String,
    id: String,
) -> Result<Option<PlaylistDetail>, String> {
    let source = parse_source(&source)?;
    let platform = state.registry.get(source).map_err(|err| err.to_string())?;
    platform.playlist(&id).await.map_err(|err| err.to_string())
}

#[tauri::command]
fn enqueue(state: State<'_, AppState>, inputs: Vec<EnqueueInput>) -> Vec<String> {
    state.engine.enqueue(inputs)
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> Vec<melodex_core::task::DownloadTask> {
    state.engine.tasks()
}

#[tauri::command]
fn pause_task(state: State<'_, AppState>, id: String) {
    state.engine.pause(&id);
}

#[tauri::command]
fn resume_task(state: State<'_, AppState>, id: String) {
    state.engine.resume(&id);
}

#[tauri::command]
fn remove_task(state: State<'_, AppState>, id: String) {
    state.engine.remove(&id);
}

#[tauri::command]
fn clear_finished(state: State<'_, AppState>) -> usize {
    state.engine.clear_finished()
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.engine.settings()
}

#[tauri::command]
fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    state.engine.update_settings(settings.clone());
    let json = serde_json::to_string_pretty(&settings).map_err(|err| err.to_string())?;
    std::fs::write(&state.settings_path, json).map_err(|err| err.to_string())?;
    Ok(())
}

/// 检查线上是否有新版本。`feed` 留空时使用内置的 GitHub Releases 地址。
#[tauri::command]
async fn check_update(
    state: State<'_, AppState>,
    feed: Option<String>,
) -> Result<melodex_core::UpdateInfo, String> {
    melodex_core::update::check(
        &state.net,
        feed.as_deref().unwrap_or(""),
        env!("CARGO_PKG_VERSION"),
    )
    .await
    .map_err(|err| err.to_string())
}

/// 用系统默认浏览器打开外部链接（更新页、安装包下载）。
/// 不走 WebView 内部导航，避免把 App 变成浏览器。
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("只允许打开 http(s) 链接".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        // 不能直接 start url：cmd 会把 & 当成命令分隔符，交给 explorer 更稳
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url.as_str()])
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    {
        let _ = url;
        Err("当前平台不支持直接打开外部链接，请手动复制下载地址".to_string())
    }
}

#[tauri::command]
fn default_download_dir(app: tauri::AppHandle) -> String {
    // 移动端没有"音乐"公共目录的通用写法，且 Android 10+ 的分区存储不允许
    // 直接写公共 Download。这里落到应用专属目录（无需任何存储权限，随卸载清理），
    // 用户也可以在设置页改成任意可写路径。
    #[cfg(mobile)]
    {
        if let Ok(dir) = app.path().app_data_dir() {
            return dir.join("Music").to_string_lossy().to_string();
        }
    }
    let _ = &app;
    melodex_core::default_download_dir()
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{path}"))
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        let parent = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path.clone());
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    // Android / iOS 没有"文件管理器定位"这一概念：把错误抛给前端，
    // 由界面提示用户在「本地曲库」里直接播放或分享。
    #[allow(unreachable_code)]
    {
        let _ = path;
        Err("当前平台不支持打开文件所在目录，请到「本地曲库」中操作".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = AppState::new(app.handle())
                .map_err(|err| -> Box<dyn std::error::Error> { Box::from(err) })?;
            let mut events = state.engine.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = events.recv().await {
                    let _ = handle.emit("melodex://event", &event);
                }
            });
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            resolve,
            lyric,
            playlist,
            enqueue,
            list_tasks,
            pause_task,
            resume_task,
            remove_task,
            clear_finished,
            get_settings,
            save_settings,
            default_download_dir,
            reveal_path,
            check_update,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("Melodex 启动失败");
}
