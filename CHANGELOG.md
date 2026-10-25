# 更新日志

本项目的版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)，
安装包命名与 `Cargo.toml` / `tauri.conf.json` 的 `version` 保持一致。

## v0.4.0

### 改进 · QQ 音乐扫码换成官方接口

v0.3.0 的扫码走的是 QQ 互联（`ptlogin2`），要经过 `skey` → `qm_keyst` 两次兑换，
中途任何一跳失败都拿不到能用的登录态，实际成功率很低。这一版整条换掉：

- 二维码由 QQ 音乐自己的 `music.login.LoginServer/CreateQRCode` 下发，
  返回 `qrcodeID` 与一张 `data:image/png;base64,...`，有效期 15 分钟；
- 扫码状态不再轮询，而是连 `wss://mu.y.qq.com/ws/handshake`，用 MQTT 5 订阅
  `management.qrcode_login/{qrcodeID}`，由服务端主动推送 `scanned` / `cookies` /
  `timeout` / `canceled` 事件（协议实现在 `login/mqtt.rs`，手写编解码并有单测）；
- 收到 `cookies` 事件后用其中的 `qqmusic_key` 调 `Login` 换正式的 `musickey`
  与数字 `musicid`，这才是可用于下载的登录态。

顺带说明为什么**不**用 HTTP 轮询状态：`GetQRCodeStatus` 实测在任何时刻都返回
`status: "scaned"`、`scanUser.userID` 恒为 0，是占位值，无法反映真实扫码进度。

### 修复 · QQ 音乐登录后仍取不到链接

取链接改用 `uin` + `authst` 字段传登录态（`comm` 与 `param` 各一份），不再拼 Cookie。
匿名请求的每个档位会返回 `result = 104003`，现在被明确归为「需要登录」。

### 新增 · 应用内在线更新

以前点「检查更新」只能打开浏览器跳到发布页自己下载。现在默认路径是应用内完成：

1. 点「立即更新」→ 内核按当前平台挑安装包（Windows 优先 NSIS `.exe`，
   Android 取 `.apk`），流式下载到缓存目录并实时回报进度；
2. 先写 `.part` 再改名，下载中断不会留下半截文件；落地前校验字节数；
3. 下载完成点「立即安装并重启」→ Windows 直接执行安装包并退出自身，
   安卓交给系统 Intent 打开安装界面。

「手动下载与其他格式」折叠区保留了逐个下载和打开发布页的入口，
自动更新失败或想自己挑格式时仍然可用。

## v0.3.0

### 新增 · QQ 音乐扫码登录

之前 QQ 音乐只能粘贴 Cookie，现在和网易云一样支持扫码：

- 申请二维码走 `ptqrshow`，这个接口返回的是 PNG 位图而不是可编码的 URL，
  所以由核心层转成 `data:` URI 交给界面 `<img>` 展示（网易云仍是本地渲染 SVG）；
- 轮询用 `ptqrtoken = hash33(qrsig)`，解析 `ptuiCB(...)` 里的状态码：
  66 等待扫码、67 已扫待确认、65/53 已失效、0 成功；
- 扫码成功后再走两跳才拿到登录态：跟随 `check_sig` 取 `uin` / `skey`，
  再调 `QQConnectLogin.LoginServer/QQLogin` 换成 `qm_keyst`。
  第二跳必须是**不跟随重定向**的请求，否则中间那一跳的 `Set-Cookie` 会丢。

### 新增 · 下载失败说清原因

以前所有失败都显示「该歌曲没有可用的音频链接」，用户无法判断是自己没登录、
缺会员，还是这首歌确实没有版权。现在失败原因被拆成六类，并在界面上给出对应操作：

| 原因 | 界面提示 | 可做什么 |
| --- | --- | --- |
| `needLogin` 需要登录 | 该音源必须登录后才提供下载链接 | 一键跳转登录 |
| `vipOnly` 会员专享 | 该音源属于会员内容 | 登录或换源 |
| `qualityMissing` 无此音质 | 该音源没有你要的音质 | 已自动降级 |
| `copyright` 已下架 | 版权方下架或当前地区不可播 | 换音源 |
| `network` 网络异常 | 网络或音源服务暂时异常 | 重试 |
| `unknown` 暂不可用 | 音源没有返回可用链接 | 重试或换源 |

判定依据是各音源自己的字段，不是猜的：

- **QQ 音乐**：匿名请求 `req_0.code = 104009`（`msg` 形如 `1.2.3.4;invalidq;`）
  → 需登录；`midurlinfo[0].result = 101404` → 该档位不存在，继续降级；
  `pneedbuy` / `tips` 非空 → 会员专享。
- **网易云**：拿不到链接时回查 `/api/v3/song/detail` 的 `privileges`，
  未登录一律归为需登录（此时 `dlLevel` 恒为 `none`、`st` 常为 `-200`，两个信号都不可信），
  登录后再用 `st = -200`（无版权）与 `fee` / `dlLevel` 区分下架与会员。
- **酷我**：`antiserver` 返回非 URL 文本时按内容归类。

### 新增 · 跨音源自动兜底

引擎以前只认入队时选中的那一个音源，于是会出现「QQ 音乐要会员 → 直接失败，
但同一首歌在酷我明明免费」的情况。现在入队时会把同一首歌在其它音源上的 ID
一并带上（`fallbacks`），首选源所有音质都拿不到链接时自动换源，
成功后任务上会标出「自动换源」并记录实际使用的音源。

这不是绕过付费墙：每个音源取的都是它自己免费开放的那一档。

### 修复 · 酷我音质标注不实

搜索结果的 `FORMATS` 字段含 `MP3H` 时被标成 320k，但 `antiserver` 的
`convert_url` 无论 `br` 传 `320kmp3` 还是 `flac`，返回的都是同一个 128k mp3
（实测晴天 269 秒对应 4.32 MB）。现在如实上报 128k，界面不再显示虚高的音质。

## v0.2.1

### 修复

- **QQ 音乐登录后仍然拿不到 vkey**：`CgiGetVkey` 的 `uin` 之前写死为 `0`，
  服务端只按匿名请求处理。现在会从 Cookie 的 `uin` / `p_uin` 解析真实账号，
  分别填入 `param.uin`（字符串）与 `comm.uin`（数字）。

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
