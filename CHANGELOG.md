# 更新日志

本项目的版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)，
安装包命名与 `Cargo.toml` / `tauri.conf.json` 的 `version` 保持一致。

## v0.2.0

### 新增 · 账号登录与本地加密凭据

未登录时大部分歌曲拿不到直链（实测：网易云 3/12、QQ 音乐 0/10、酷我 10/10），
登录从"锦上添花"变成"能不能用"的问题。

- **网易云二维码登录**：官方 `login/qrcode` 接口，手机扫码确认，全程不接触账号密码；
  状态机覆盖 等待扫码 / 已扫码待确认 / 成功 / 过期，二维码由 Rust 侧渲染成 SVG
- **QQ 音乐 Cookie 导入**：官方未开放扫码接口，界面给出取 Cookie 的步骤
- **酷我**：免登录，界面直接标注
- **本地加密保险库**（`vault.rs`）：AES-256-GCM 落盘，随机主密钥单独存放；
  可选 PIN，用 Argon2id 派生 KEK 包裹主密钥，没 PIN 解不开
- 界面只显示「账号昵称 + Cookie 键名」，接口层从不回传 Cookie 值
- 设置页新增「账号」区块：登录状态、扫码/导入入口、退出登录、PIN 管理

### 修复

- 设置页与弹窗用错设计令牌（`--bg-1` / `--bg-2` 不存在）导致弹窗背景透明，
  已统一为 `--surface` / `--surface-2`
- 移动端账号行被按钮撑高，改为同行布局

## v0.1.1

### 修复

- **点击「下载」立即崩溃**（v0.1.0 的致命缺陷）
  Tauri 的同步 command 运行在主线程，那里没有 Tokio 运行时上下文；而
  `DownloadEngine::enqueue` 内部的 `tokio::spawn` 会因此 panic
  （`there is no runtime running`），panic 穿过 WebView 的 IPC 回调后整个进程被 abort，
  表现为窗口瞬间消失。同类问题也影响「继续 / 暂停 / 移除 / 清空」这些会触发调度的操作。

  两处改动：
  1. 会触发调度的 command 全部改为 `async fn`，由 `tauri::async_runtime` 在运行时内执行；
  2. 任务派发改为三级兜底（当前运行时 → 缓存的 `Handle` → 独立线程自建单线程运行时），
     即使将来有人从无上下文的线程调用也不会 panic。

- 新增崩溃日志：panic 会追加写入应用数据目录的 `crash.log`（Windows 为
  `%APPDATA%\app.melodex\crash.log`），不再"窗口消失、毫无线索"。

### 新增

- 回归测试 `spawning_outside_runtime_does_not_panic`：在无运行时的线程上派发任务，
  确保不会再退回裸 `tokio::spawn`。

## v0.1.0

首个可安装版本。

- 多源聚合搜索：网易云音乐、QQ 音乐、酷我音乐，按「标题 + 歌手 + 时长」指纹合并同一首歌
- 逐源可用性报告，单源失败不影响整体结果
- 下载：队列 + 并发调度、HTTP `Range` 断点续传、音质自动降级、重复文件策略
- 落盘：命名模板、ID3v2.3 标签、内嵌封面、LRC 歌词
- 界面：Vue 3 双主题响应式布局，桌面侧栏 / 移动底部标签栏
- 在线更新：启动静默检查 GitHub Releases，手动确认后下载
- 发行：Windows（NSIS / MSI）与 Android（通用 APK，固定签名可覆盖升级）
