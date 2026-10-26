//! Tauri 外壳：把 Rust 核心的能力暴露成 command，并转发引擎事件给前端。
//!
//! 这一层刻意保持"薄"：不做业务判断，只做参数转换与错误字符串化，
//! 所有逻辑都在 `melodex-core` 里，便于单独测试。

use std::path::PathBuf;
use std::sync::Mutex;

use melodex_core::vault::{Account, AccountInfo, Opened, Vault, VaultData, MIN_PIN_LEN};
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
    creds: CredStore,
    /// 加密保险库所在目录
    vault_dir: PathBuf,
    /// `None` 表示未解锁（启用了 PIN 但还没输入）
    vault: Mutex<Option<Vault>>,
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
        let vault_dir = dir.join("vault");

        // 未被 PIN 保护时自动解锁并装载凭据；启用了 PIN 则等用户输入
        let pin_protected = Vault::is_pin_protected(&vault_dir);
        let mut unlocked = None;
        if !pin_protected {
            if let Ok(Opened::Ready(vault)) = Vault::open(&vault_dir) {
                if let Ok(data) = vault.load() {
                    load_into(&creds, &data);
                }
                unlocked = Some(vault);
            }
        }

        let registry = Registry::new(net.clone(), creds.clone());
        let engine = DownloadEngine::new(registry.clone(), settings, Some(dir.join("tasks.json")))
            .map_err(|err| err.to_string())?;
        let aggregator = Aggregator::new(registry.clone());

        Ok(Self {
            aggregator,
            registry,
            engine,
            net,
            settings_path,
            creds,
            vault_dir,
            vault: Mutex::new(unlocked),
        })
    }

    /// 把保险库里的 Cookie 注入各音源适配器。
    fn reload_creds(&self) {
        let Ok(data) = self.load_vault() else { return };
        load_into(&self.creds, &data);
    }

    fn load_vault(&self) -> Result<VaultData, String> {
        let guard = self
            .vault
            .lock()
            .map_err(|_| "凭据库状态异常".to_string())?;
        let vault = guard
            .as_ref()
            .ok_or_else(|| "凭据库未解锁，请先输入 PIN".to_string())?;
        vault.load().map_err(|err| err.to_string())
    }

    /// 写入一条账号：同时进内存 CredStore 与加密保险库。
    fn store_account(
        &self,
        source: Source,
        cookie: &str,
        nickname: Option<String>,
    ) -> Result<AccountInfo, String> {
        let cookie = cookie.trim();
        if cookie.is_empty() {
            return Err("Cookie 不能为空".into());
        }
        let mut data = self.load_vault()?;
        let account = Account::new(cookie, nickname);
        data.set(source.id(), account.clone());
        {
            let guard = self
                .vault
                .lock()
                .map_err(|_| "凭据库状态异常".to_string())?;
            let vault = guard
                .as_ref()
                .ok_or_else(|| "凭据库未解锁，请先输入 PIN".to_string())?;
            vault.save(&data).map_err(|err| err.to_string())?;
        }
        self.creds.set(source, Some(account.cookie.clone()));
        Ok(AccountInfo::of(source.id(), &account))
    }

    fn status(&self) -> VaultStatus {
        let unlocked = self
            .vault
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false);
        let accounts = if unlocked {
            self.load_vault()
                .map(|data| data.infos())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        VaultStatus {
            exists: Vault::exists(&self.vault_dir),
            pin_protected: Vault::is_pin_protected(&self.vault_dir),
            unlocked,
            accounts,
            min_pin_len: MIN_PIN_LEN,
        }
    }
}

fn load_into(creds: &CredStore, data: &VaultData) {
    for (id, account) in data.accounts.iter() {
        if let Some(source) = Source::from_id(id) {
            creds.set(source, Some(account.cookie.clone()));
        }
    }
}

/// 凭据库状态，供设置页展示。不含任何 Cookie 明文。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultStatus {
    exists: bool,
    pin_protected: bool,
    unlocked: bool,
    accounts: Vec<AccountInfo>,
    min_pin_len: usize,
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
    Ok(state
        .aggregator
        .search(&keyword, page, limit, &targets)
        .await)
}

#[tauri::command]
async fn resolve(
    state: State<'_, AppState>,
    source: String,
    id: String,
    quality: Option<String>,
) -> Result<PlayUrl, String> {
    let source = parse_source(&source)?;
    let level = quality
        .as_deref()
        .and_then(melodex_core::quality::QualityLevel::parse)
        .unwrap_or_else(|| state.engine.settings().quality_of(source));
    let item = melodex_core::quality::find(source, level)
        .ok_or_else(|| format!("{} 没有可用音质档位", source.label()))?;
    let platform = state.registry.get(source).map_err(|err| err.to_string())?;
    let variant = Variant::new(source, id, String::new());
    platform
        .resolve(&variant, item)
        .await
        .map_err(|err| err.to_string())
}

/// 各音源的音质档位表。界面按平台分栏渲染，名称直接照抄平台自己的叫法。
#[tauri::command]
fn platform_qualities() -> Vec<melodex_core::quality::PlatformQuality> {
    melodex_core::quality::catalog()
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

/// 入队。
///
/// 必须是 `async`：Tauri 的同步 command 运行在主线程上，那里没有 Tokio 运行时
/// 上下文，而 `DownloadEngine::enqueue` 会派发下载任务。异步 command 由
/// `tauri::async_runtime` 在运行时内执行，`tokio::spawn` 才有落脚点。
///
/// 返回值用 `Result` 包裹，是因为 async command 借用 `State<'_, _>` 时
/// Tauri 依赖 `Result` 分支来正确处理生命周期。
#[tauri::command]
async fn enqueue(
    state: State<'_, AppState>,
    inputs: Vec<EnqueueInput>,
) -> Result<Vec<String>, String> {
    Ok(state.engine.enqueue(inputs))
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> Vec<melodex_core::task::DownloadTask> {
    state.engine.tasks()
}

#[tauri::command]
async fn pause_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine.pause(&id);
    Ok(())
}

#[tauri::command]
async fn resume_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine.resume(&id);
    Ok(())
}

#[tauri::command]
async fn remove_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine.remove(&id);
    Ok(())
}

#[tauri::command]
async fn clear_finished(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.engine.clear_finished())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.engine.settings()
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    state.engine.update_settings(settings.clone());
    let json = serde_json::to_string_pretty(&settings).map_err(|err| err.to_string())?;
    std::fs::write(&state.settings_path, json).map_err(|err| err.to_string())?;
    Ok(())
}

/// 凭据库状态（是否存在、是否启用 PIN、是否已解锁、已保存哪些账号）。
#[tauri::command]
fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    state.status()
}

/// 用 PIN 解锁凭据库，解锁后立即把 Cookie 注入各音源。
#[tauri::command]
fn vault_unlock(state: State<'_, AppState>, pin: String) -> Result<VaultStatus, String> {
    let vault = Vault::open_with_pin(&state.vault_dir, &pin).map_err(|err| err.to_string())?;
    {
        let mut guard = state
            .vault
            .lock()
            .map_err(|_| "凭据库状态异常".to_string())?;
        *guard = Some(vault);
    }
    state.reload_creds();
    Ok(state.status())
}

/// 设置 / 取消 PIN。传 `null` 表示取消保护。
#[tauri::command]
fn vault_set_pin(state: State<'_, AppState>, pin: Option<String>) -> Result<VaultStatus, String> {
    {
        let guard = state
            .vault
            .lock()
            .map_err(|_| "凭据库状态异常".to_string())?;
        let vault = guard
            .as_ref()
            .ok_or_else(|| "凭据库未解锁，请先输入 PIN".to_string())?;
        vault
            .set_pin(pin.as_deref())
            .map_err(|err| err.to_string())?;
    }
    Ok(state.status())
}

/// 申请扫码登录的二维码。目前仅网易云支持（官方二维码接口）。
#[tauri::command]
async fn login_qr_start(
    state: State<'_, AppState>,
    source: String,
) -> Result<melodex_core::login::QrTicket, String> {
    let source = parse_source(&source)?;
    match source {
        Source::Netease => melodex_core::login::netease::start(&state.net)
            .await
            .map_err(|err| err.to_string()),
        Source::Qq => melodex_core::login::qq::start(&state.net)
            .await
            .map_err(|err| err.to_string()),
        Source::Kuwo => Err("酷我音乐可以免登录下载，无需登录".into()),
    }
}

/// 轮询扫码结果；确认成功后立刻写入加密保险库并注入音源。
#[tauri::command]
async fn login_qr_poll(
    state: State<'_, AppState>,
    source: String,
    key: String,
) -> Result<melodex_core::login::QrPoll, String> {
    let source = parse_source(&source)?;
    let poll = match source {
        Source::Netease => melodex_core::login::netease::poll(&state.net, &key).await,
        Source::Qq => melodex_core::login::qq::poll(&state.net, &key).await,
        Source::Kuwo => Err(melodex_core::error::Error::Invalid(
            "酷我音乐可以免登录下载，无需登录".into(),
        )),
    }
    .map_err(|err| err.to_string())?;

    if poll.status == melodex_core::login::QrStatus::Confirmed {
        if let Some(cookie) = poll.cookie.clone() {
            // 保存失败要让用户知道，否则会出现"看起来登录了但下载仍然受限"
            state
                .store_account(source, &cookie, poll.nickname.clone())
                .map_err(|err| format!("登录成功，但凭据保存失败：{err}"))?;
        }
    }
    Ok(poll)
}

// ---------------------------------------------------------------------------
// 内置网页登录
//
// 手机端和桌面端共用一套「打开平台网页 → 用户自己登录 → 读回 Cookie」的流程，
// 但读 Cookie 的能力只有桌面有（Tauri 的 `cookies()` 在 Android 恒返回空），
// 所以手机端仍然引导用户扫码，桌面端默认网页登录。
// ---------------------------------------------------------------------------

/// 网页登录窗口的固定标签，重复点击复用一个窗口，避免开出一堆。
#[cfg_attr(not(desktop), allow(dead_code))]
const LOGIN_WINDOW: &str = "login-web";
/// 网页登录的最长等待时间。QQ / 网易的登录态一般一两分钟就完成，
/// 超过这个时间用户多半已经走开，继续挂着只是浪费轮询。
#[cfg_attr(not(desktop), allow(dead_code))]
const LOGIN_TIMEOUT_SECS: u64 = 300;

/// 手机端的内置浏览器读不到 HttpOnly Cookie，网页登录只在桌面提供。
#[tauri::command]
fn login_web_supported() -> bool {
    cfg!(desktop)
}

/// 打开内置登录窗口。登录成功后会自己在后台读凭据、存库、关窗。
#[cfg(desktop)]
#[tauri::command]
async fn login_web_start(app: tauri::AppHandle, source: String) -> Result<(), String> {
    let source = parse_source(&source)?;
    start_web_login(app, source)
}

/// 手机端：Tauri 的 `cookies()` 在 Android 恒返回空，网页登录拿不到凭据，
/// 所以直接引导用户走扫码，而不是给一个必然失败的按钮。
#[cfg(not(desktop))]
#[tauri::command]
async fn login_web_start(app: tauri::AppHandle, source: String) -> Result<(), String> {
    let _ = (app, source);
    Err("手机端的内置浏览器读不到登录凭据，请改用扫码登录".into())
}

#[cfg(desktop)]
fn start_web_login(app: tauri::AppHandle, source: Source) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let home = match source {
        Source::Qq => "https://y.qq.com/",
        Source::Netease => "https://music.163.com/",
        Source::Kuwo => return Err("酷我音乐免登录即可下载，不需要登录".into()),
    };
    let parsed = tauri::Url::parse(home).map_err(|err| format!("登录地址无效：{err}"))?;

    // 已有窗口时只把它提到前面，不重复创建
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(&app, LOGIN_WINDOW, WebviewUrl::External(parsed))
        .title(format!("登录 {}（登录后自动关闭）", source.label()))
        .inner_size(430.0, 720.0)
        .min_inner_size(360.0, 520.0)
        .center()
        .build()
        .map_err(|err| format!("无法打开登录窗口：{err}"))?;

    tauri::async_runtime::spawn(async move {
        watch_web_login(app, window, source).await;
    });
    Ok(())
}

/// 轮询登录窗口的 Cookie，一旦出现该平台的鉴权字段就落库。
///
/// 用轮询而不是监听导航：平台登录成功后不一定会跳转，可能只是原地刷新，
/// 只看 Cookie 有没有出现最可靠。
#[cfg(desktop)]
async fn watch_web_login(app: tauri::AppHandle, window: tauri::WebviewWindow, source: Source) {
    use std::time::{Duration, Instant};

    let home = match source {
        Source::Qq => "https://y.qq.com/",
        Source::Netease => "https://music.163.com/",
        Source::Kuwo => "https://www.kuwo.cn/",
    };
    let Ok(scope) = tauri::Url::parse(home) else {
        return;
    };

    let deadline = Instant::now() + Duration::from_secs(LOGIN_TIMEOUT_SECS);
    loop {
        if Instant::now() > deadline {
            let _ = app.emit("login-web-error", "等待登录超时，请重新发起登录");
            close_login_window(&app);
            return;
        }
        tokio::time::sleep(Duration::from_millis(1200)).await;

        // 用户自己把窗口关了，就别再轮询
        if app.get_webview_window(LOGIN_WINDOW).is_none() {
            let _ = app.emit("login-web-closed", source.id());
            return;
        }

        let cookies = match read_window_cookies(&window, scope.clone()).await {
            Ok(list) => list,
            // 窗口刚建好时读不到 Cookie 库是正常的，继续等下一轮
            Err(_) => continue,
        };
        let Some(cookie) = collect_cookie(source, &cookies) else {
            continue;
        };

        let state = app.state::<AppState>();
        let nickname = guess_nickname(source, &cookie).or_else(|| match source {
            Source::Netease => Some("网易云用户".into()),
            _ => None,
        });
        let saved = state
            .store_account(source, &cookie, nickname)
            .map_err(|err| err.to_string());
        match saved {
            Ok(info) => {
                let _ = app.emit("login-web-done", &info);
            }
            Err(err) => {
                let _ = app.emit("login-web-error", err);
            }
        }
        close_login_window(&app);
        return;
    }
}

#[cfg(desktop)]
fn close_login_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = window.close();
    }
}

/// 读窗口里的 Cookie。
///
/// Windows 上 `cookies_for_url` 在主线程调用会死锁（WebView2 的已知问题），
/// 所以必须丢到阻塞线程池，让主线程继续跑消息循环。
#[cfg(desktop)]
async fn read_window_cookies(
    window: &tauri::WebviewWindow,
    url: tauri::Url,
) -> Result<Vec<(String, String)>, String> {
    let window = window.clone();
    tauri::async_runtime::spawn_blocking(move || {
        window
            .cookies_for_url(url)
            .map(|list| {
                list.into_iter()
                    .map(|item| (item.name().to_string(), item.value().to_string()))
                    .collect::<Vec<_>>()
            })
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| format!("读取登录状态失败：{err}"))?
}

/// 从 Cookie 罐里挑出鉴权必需的字段，拼成平台适配器认识的格式。
///
/// 只留关键项：`pgv_pvid` 这类统计 Cookie 存进去没有任何用处，
/// 还会让保险库里的密文白白变长。
#[cfg_attr(not(desktop), allow(dead_code))]
fn collect_cookie(source: Source, cookies: &[(String, String)]) -> Option<String> {
    fn get<'a>(cookies: &'a [(String, String)], name: &str) -> Option<&'a str> {
        cookies
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .filter(|value| !value.is_empty())
    }
    match source {
        Source::Qq => {
            let uin = get(cookies, "uin")
                .or_else(|| get(cookies, "p_uin"))
                .or_else(|| get(cookies, "wxuin"))?;
            let key = get(cookies, "qm_keyst").or_else(|| get(cookies, "qqmusic_key"))?;
            Some(format!("uin={uin}; qm_keyst={key}; qqmusic_key={key}"))
        }
        Source::Netease => {
            let token = get(cookies, "MUSIC_U")?;
            let csrf = get(cookies, "__csrf").unwrap_or_default();
            Some(format!("MUSIC_U={token}; __csrf={csrf}"))
        }
        // 酷我不需要登录，永远不会走到这里
        Source::Kuwo => None,
    }
}

/// 手动保存 Cookie。扫码失败，或用户直接从浏览器复制时走这条路。
#[tauri::command]
fn account_save(
    state: State<'_, AppState>,
    source: String,
    cookie: String,
    nickname: Option<String>,
) -> Result<AccountInfo, String> {
    let source = parse_source(&source)?;
    let nickname = nickname.or_else(|| guess_nickname(source, &cookie));
    state.store_account(source, &cookie, nickname)
}
/// 退出登录：清除内存与保险库中的该音源凭据。
#[tauri::command]
fn account_clear(state: State<'_, AppState>, source: String) -> Result<VaultStatus, String> {
    let source = parse_source(&source)?;
    let cleared = {
        let mut data = state.load_vault()?;
        if data.remove(source.id()) {
            let guard = state
                .vault
                .lock()
                .map_err(|_| "凭据库状态异常".to_string())?;
            let vault = guard.as_ref().ok_or_else(|| "凭据库未解锁".to_string())?;
            vault.save(&data).map_err(|err| err.to_string())?;
            true
        } else {
            false
        }
    };
    if cleared {
        state.creds.set(source, None);
    }
    Ok(state.status())
}

/// QQ 的 Cookie 里带 `uin`，可以据此显示一个可读的账号名。
fn guess_nickname(source: Source, cookie: &str) -> Option<String> {
    if source != Source::Qq {
        return None;
    }
    let uin = melodex_core::cookie::value_of(cookie, "uin")
        .or_else(|| melodex_core::cookie::value_of(cookie, "p_uin"))?;
    let digits = uin.trim_start_matches(['o', 'u', '_']);
    Some(format!("QQ {digits}"))
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

/// 在线更新的进度事件载荷。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: u64,
}

/// 把安装包下载到本地缓存目录，边下边往前端推进度。
///
/// 不传 `asset_name` 时按当前平台挑最合适的安装包；传了则按名字精确匹配
/// （例如用户手动选择了 `.msi`）。返回本地绝对路径，交给 `update_install` 拉起。
#[tauri::command]
async fn update_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    info: melodex_core::UpdateInfo,
    asset_name: Option<String>,
) -> Result<String, String> {
    let asset = match asset_name.as_deref() {
        Some(name) => info
            .assets
            .iter()
            .find(|asset| asset.name == name)
            .ok_or_else(|| format!("这次发布里没有名为 {name} 的安装包"))?,
        None => melodex_core::update::pick_asset(&info.assets)
            .ok_or_else(|| "这次发布没有适用于当前平台的安装包".to_string())?,
    };

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|err| format!("无法定位缓存目录：{err}"))?
        .join("updates");

    let emitter = app.clone();
    let path = melodex_core::update::download_asset(
        &state.net,
        &asset.url,
        &dir,
        &asset.name,
        asset.size,
        |downloaded, total| {
            let _ = emitter.emit("update-progress", UpdateProgress { downloaded, total });
        },
    )
    .await
    .map_err(|err| err.to_string())?;

    Ok(path.to_string_lossy().into_owned())
}

/// 拉起刚下载的安装包。
///
/// Windows 上直接执行 NSIS 安装包，装完即完成升级；应用自身要先退出，
/// 否则安装器无法覆盖正在运行的 exe。安卓交给系统 Intent 打开，会弹安装界面。
#[tauri::command]
async fn update_install(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let file = std::path::Path::new(&path);
    if !file.exists() {
        return Err("安装包不存在，请重新下载".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(file)
            .spawn()
            .map_err(|err| format!("无法启动安装程序：{err}"))?;
        // 给前端留出渲染「正在退出」的时间，再结束进程
        let handle = app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            handle.exit(0);
        });
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_path(&path, None::<&str>)
            .map_err(|err| format!("无法打开安装包：{err}"))?;
        Ok(())
    }
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

/// 安装崩溃日志钩子。
///
/// Windows 释放版没有控制台，panic 穿过 WebView 回调时进程直接 abort，
/// 用户只看到窗口消失、毫无线索。这里把所有 panic（含线程名、位置、消息）
/// 追加写入应用数据目录的 `crash.log`，便于回顾定位。
fn install_panic_hook(dir: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let stamp = melodex_core::task::now_secs();
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let text = format!("[{stamp}] thread={name} {info}\n");
        eprintln!("{text}");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("crash.log"))
        {
            use std::io::Write;
            let _ = file.write_all(text.as_bytes());
        }
        previous(info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 先装钩子，后面任何初始化失败也能留下记录
            if let Ok(dir) = app.path().app_data_dir() {
                install_panic_hook(dir);
            }
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
            vault_status,
            vault_unlock,
            vault_set_pin,
            login_qr_start,
            login_qr_poll,
            login_web_supported,
            login_web_start,
            platform_qualities,
            account_save,
            account_clear,
            default_download_dir,
            reveal_path,
            check_update,
            update_download,
            update_install,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("Melodex 启动失败");
}
