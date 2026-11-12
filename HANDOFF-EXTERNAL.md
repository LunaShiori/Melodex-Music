# Melodex Music 交接文档（面向接手模型）

> 生成时间：2026-11-12 · 生成者：前一位负责本项目的 agent
> 目的：让接手方在不读历史对话的前提下，能在 20 分钟内跑起来、30 分钟内做出第一个可交付改动。

---

## 0. 一句话

Melodex Music 是一个 Tauri 2 + Vue 3 的桌面/安卓音乐客户端，Rust 核心负责从网易云、QQ 音乐、酷我三个音源聚合搜索、取播放直链、下载落盘。代码在 `D:\AI\dev\Melodex-Music`，远端 `git@github.com:LunaShiori/Melodex-Music.git`，主干 `main`。

## 1. 现状快照

| 项 | 值 |
| --- | --- |
| 最新提交 | `efbfb86 chore(release): 2.0.1 — Android 更新包安装授权、macOS 通用 dmg` |
| 版本号 | 2.0.1（`package.json` / `src-tauri/tauri.conf.json` / `Cargo.toml` / `src-tauri/Cargo.toml` 四处一致） |
| 最新 tag / Release | `v2.0.1`（GitHub 上 latest，6 个产物：exe、msi、apk、deb、AppImage、universal.dmg） |
| 工作区状态 | clean，无未提交改动 |
| 测试基线 | `cargo test --features net` = 157 + 16 passed；`cargo test -p melodex-core --no-default-features` = 86 passed |
| 前端校验 | `npm run typecheck`、`npm run build` 均通过 |

## 2. 架构地图

```
crates/melodex-core/src/
  model.rs        Track / Variant / normalize_title / version_tag / VersionClass
  aggregator.rs   group_tracks：跨源合并同一首歌（按 version_tag 分层判重）
  net.rs          Net + ProxyFor{Search,Link,Download,Update} + ProxyScopes
  settings.rs     Settings（含 speed_limit_kbps、proxy、proxy_scopes）
  error.rs        Error + Reason（UI 按 Reason 给可操作提示）
  quality.rs      音质档位与 needs_login()
  download/       engine.rs（调度/重试/续传）、transfer.rs（HTTP 流式下载 + 限速）
  login/          netease.rs、qq.rs（扫码与 Cookie）
  platforms/      netease.rs、qq.rs、kuwo.rs（搜索、取链、歌单、歌词）
  stream.rs       本地回环代理：给播放器请求补 Referer
  update.rs       应用内更新检查与资产选择
  vault.rs / cookie.rs  凭据加密存储（AES-256-GCM）

src/
  views/          Search / Recent / Downloads / Library / Playlists / Settings
  components/     PlayerBar、LyricView、DownloadPicker、QueuePanel、TrackRow、TabBar…
  stores/         player、tasks、search、accounts、update 等 Pinia store
  lib/transport/  tauri.ts（真实调用）与 demo.ts（浏览器演示模式，无 Tauri 时自动启用）
  lib/history.ts  最近搜索 / 最近播放（localStorage）
src-tauri/
  src/lib.rs                 壳命令：网页登录窗口、save_text_file、APK 安装
  android/ApkInstallPlugin.kt 安卓应用内安装（FileProvider + ClipData + 逐个授权）
  tauri.conf.json             bundle targets: nsis / msi
```

## 3. 版本历史（与本轮需求相关的关键提交）

| 版本 | 主要改动 |
| --- | --- |
| v1.6.x | 单任务限速（可自定义，上限 1 GB/s） |
| v1.7.x | 全局代理；歌单整体导出 / 批量导入（JSON，去重入队） |
| v1.8.0 | 代理按协议 + 用途分流；原唱只认平台官方字段；酷我 Cookie 登录；播放队列面板；搜索历史 |
| v1.9.0 | 下载 403 修复（补 Referer + 403/404 映射 LinkRejected 并重取链接）；歌曲右键菜单；播放模式；音量百分比；「最近」页签 |
| v2.0.0 | 伴奏/原唱判重修复（version_tag）；macOS + Linux 打包；新图标；下载音质弹窗可用性与反馈 |
| v2.0.1 | 安卓应用内安装授权修复；macOS universal dmg |

## 4. 环境与工具链陷阱（先读这节，能省半天）

1. **仓库必须放在 ASCII 路径**。路径含中文时，MinGW 的 `ld.exe`（ANSI 版）链接会报 `cannot find ...rcgu.o` / `cannot open output file`，但 `gcc` 单文件编译正常。
2. **先 `source .local/env.sh`**，里面配置了 Rust/Node 工具链。注意 `PATH` 必须用 `/d/AI/...` 形式，`D:/AI/...` 会被冒号截断。
3. **`src-tauri` 编译需要 `TMP`/`TEMP` 指向仓库内**：`export TMP=D:/AI/dev/Melodex-Music/.local/tmp TEMP=$TMP`，否则 `ring` 的 C 构建报 `Cannot create temporary file in C:\Users\...\AppData\Local\Temp: Permission denied`。
4. **推送前必须本地跑全**：
   - `cargo fmt --check`
   - `cargo clippy --features net --all-targets -- -D warnings`
   - `cargo test --features net`
   - `cargo test -p melodex-core --no-default-features`（CI 单独跑这条；曾因 `is_retryable` 的 `#[cfg(not(feature="net"))]` 分支漏了 `Error::HttpStatus`，本地全绿、CI 红）
   - `npm run typecheck` + `npm run build`
5. **改动 `package.json` 的版本号后必须重启 dev server**，`__APP_VERSION__` 是 vite 在启动时注入的，否则演示模式里更新提示会显示旧版本。启动：`npm run dev -- --port 5183`。
6. **发版流程**：四处版本号 → `cargo check` 刷新 `Cargo.lock` → 写 CHANGELOG 段 → 提交 → 推 `main` → `git tag -a vX.Y.Z -m "..."` 并推 tag。release workflow 的 job 顺序：`windows(nsis,msi) → android(apk) → linux(deb,appimage) → macos(universal dmg) → publish`；`publish` 用 `awk` 从 `CHANGELOG.md` 抽对应段落拼进 Release 正文。
7. **本机访问 github.com 直链经常超时**（资产下载几乎不可用），只能走 `api.github.com` 读元数据。`.local/github-token.txt` 里的 fine-grained token 已过期（返回 `Bad credentials`），涉及 Actions/API 写操作时需要用户重新签发。
8. **安卓相关有两套脚本必须同步改**：`src-tauri/android/ApkInstallPlugin.kt` 与 `scripts/android-apk-plugin.py`（后者负责注入 `<queries>`、FileProvider 与 `androidx.core` 依赖）。图标由 `scripts/android-icons.py` 从 `src-tauri/icons/android` 搬到 `gen/android`。
9. **图标**：源图由脚本生成后 `npx tauri icon` 会重置安卓自适应图标的背景色并让 foreground 变成整张图，需要按「背景 drawable 渐变 + 前景只有字形、限 66% 安全区」重新出 5 个密度。
10. **录屏/截图工具**：`.local/tools/shot.mjs`（静态截图）、`.local/tools/record.mjs`（CDP 逐帧录屏）。注意录屏窗口尺寸必须 `W×H + (30,129)` 且不要混用设备仿真，否则帧尺寸会不一致。

## 5. 代码约定

- 注释与文档用中文，**语气标准化**，不用口语（用户明确要求过；新写的注释不要出现「坑」「顺手」「免得」这类词）。
- UI 选中态必须用 `reactive`/`ref` 承载，普通对象不会触发更新（已在 `DownloadPicker` 上踩过）。
- 错误统一走 `Error` → `Reason`，UI 按 Reason 给可操作提示（例如 `LinkRejected` 提示重试或换源）。
- 平台适配**只采信官方字段**，缺字段不显示、不猜（原唱标签即此原则）。
- 歌曲判重按 `version_tag`（命中的标记原文，如「伴奏」「Live」）而不是 `VersionClass` 大类，否则「伴奏」与「纯音乐」会被并成一条。
- 代理：`ProxyFor::Link` 用于取播放直链，其余走 `Search`；下载器不走 `Net`，而是自己 `build_client` 并带 `stream_headers()`（QQ 需要 Referer）。
- **目前没有 i18n 框架**，所有文案硬编码中文。双语化第一步是引入 i18n 并把文案抽到词条表。

## 6. 已知缺陷与定位线索

| 现象 | 位置线索 |
| --- | --- |
| 网易云歌单只显示 10 首，无「加载更多」 | `platforms/netease.rs` 歌单接口未翻页；`PlaylistsView.vue` 无分页 UI |
| 歌单里下载不能选音质 | `PlaylistsView.vue` 的批量下载直接 `tasks.enqueue`，绕过了 `DownloadPicker` |
| QQ 音乐登录经常掉 | `login/qq.rs`、`cookie.rs`、`vault.rs`：`qm_keyst` 过期与凭据刷新时机 |
| 清空下载记录后曲库也空了 | `LibraryView.vue` 的数据源是 `tasks` 的已完成项，清空任务即清空曲库；应改为以磁盘扫描为准 |
| 重复歌曲无提示 | `aggregator.rs` 合并后同一条目多 variant，UI 未区分「同源重复 / 不同版本」 |
| 歌词页顶部歌名歌手未居中 | `LyricView.vue` 头部布局 |

## 7. 能力边界（重要，不要承诺做不到的事）

- **Spotify**：Web API 可做搜索/元数据/歌单；播放需官方 SDK + Premium 授权；**没有下载接口**，音频受 DRM 保护。
- **Apple Music**：iTunes Search API 可做搜索 + 30 秒试听（`previewUrl`）；MusicKit 需 99 美元/年开发者账号，全曲走 FairPlay，**无明文音频**。
- **YouTube Music**：没有官方音乐流 API；YouTube Data API 只有元数据。
- **酷我扫码登录**：已实测不可行（`passport.kuwo.cn/ptqrlogin` 502、`/api/login/qr` 与 `/api/user/qrcode` 404、`www.kuwo.cn/login` 302 回首页），当前走「桌面内置登录窗口 + 手机粘贴 Cookie」。
- **合规底线**：绕过 DRM（解密、伪造许可证、抓取加密流）属于规避技术保护措施，不做。「下载后 24 小时内删除」这类声明不改变侵权性质，可以作为提示，但不能当作授权。

## 8. 待办清单（用户 2026-11-12 提出）

**A. 工程与质量**
1. 细节重构，消除重复与复杂代码
2. 文案词条统一优化
3. 注释语气标准化（去口语）
**B. 双语**
4. 软件中英双语
5. GitHub README / 截图 / 图片文字双语
**C. 平台**
6. 在合规前提下接入 Spotify、YouTube Music、Apple Music；不可用的功能必须有明确提示
7. 接入酷狗
8. 酷我二维码登录（类 QQ 音乐流程）
9. 代理可按平台选择（哪些平台走代理、哪些直连）
**D. 播放器能力**
10. 歌词页顶部歌名与歌手居中（桌面 + 手机）
11. 安卓「灵动岛」（注意：安卓无对应系统能力，只能是悬浮窗或通知样式，需产品决策）
12. 应用可作为默认播放器打开其他软件里的音频（文件关联 / 打开方式）
13. 读取电脑与手机本地音频（本地曲库扫描）
**E. 缺陷**
14. 网易云歌单只显示 10 首、无刷新与加载更多
15. 新增「搜歌手」入口
16. 歌单内下载可选音质
17. QQ 音乐登录频繁掉线
18. 清空下载记录不应连带清空曲库
19. 重复歌曲（非重复歌手/版本）给出提示
**F. 合规文案**
20. 加入「请于下载后 24 小时内删除」等声明

## 9. 常用命令

```bash
cd "D:/AI/dev/Melodex-Music"
source .local/env.sh
export TMP=D:/AI/dev/Melodex-Music/.local/tmp TEMP=$TMP

npm run typecheck && npm run build
cargo fmt --check
cargo clippy --features net --all-targets -- -D warnings
cargo test --features net
cargo test -p melodex-core --no-default-features
npm run dev -- --port 5183      # 浏览器演示模式
git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z
```

## 10. 建议的上手顺序

1. 先按第 4 节把环境和校验跑通（约 15 分钟），确认基线全绿。
2. 从第 6 节的缺陷清单里挑 2–3 个可独立验证的修掉，建立对代码的直觉。
3. 再做双语化与重构（影响面大，需要全量回归）。
4. 平台接入放在最后，并且先和需求方确认「搜索/试听/迁移」与「全曲播放下载」的边界。
