# Melodex Music 交接文档（面向接手的模型 / 人）

> 最后更新：2026-11-15，对应版本 **v2.1.1**（HEAD `5730a61`）
> 上一版生成于 2026-11-12（v2.0.1 时代），内容已整体重写；更细的中文工程笔记在同目录 `HANDOFF.md`（被 git 排除，只在本地）。

---

## 0. 一句话

**Melodex Music** 是一个多源音乐聚合客户端：一次搜索同时打网易云 / QQ / 酷我 / 酷狗，同曲合并成一行，
内置播放器 + 逐行歌词，可选音质下载，落盘写 ID3v2.3 标签。
技术栈：Rust core（`crates/melodex-core`）+ Tauri 2 外壳（`src-tauri`）+ Vue 3/TS 前端（`src`），一套代码出 Windows / macOS / Linux / Android。

---

## 1. 位置与身份

| 项 | 值 |
| --- | --- |
| 工作目录 | `D:\AI\dev\Melodex-Music`（**必须 ASCII 路径**，带中文会让 MinGW 链接失败） |
| 远端 | `git@github.com:LunaShiori/Melodex-Music.git`（SSH，可直接 push） |
| 分支 / HEAD | `main` / `5730a61` |
| 最新 tag | `v2.1.1`（GitHub `releases/latest` 也指向它） |
| 旧目录 | `D:\AI\.workbuddy\MusicTools\melodex` 是只读备份，别再往那儿写 |
| 版本号四处 | `package.json`、`src-tauri/tauri.conf.json`、`Cargo.toml:7`、`src-tauri/Cargo.toml:3`，改完跑 `cargo check` 刷新 `Cargo.lock` |
| 仓库可见性 | public |
| 已发布产物 | exe 3.9MB / msi 5.9MB / apk 69.8MB / dmg 12.1MB / deb 7.5MB / AppImage 81.7MB |

**本机网络限制（很重要）**：`github.com` 网页与 release 资产直链被墙（下载极慢/超时），
但 `api.github.com` 可匿名读、SSH push 正常。`.local/github-token.txt` 里的 fine-grained token **已过期**，
所以 Actions 日志、Release 写操作都拿不到——需要的话只能找用户要新 token。

---

## 2. 立刻验一遍（先确认环境是好的）

```bash
cd "D:/AI/dev/Melodex-Music"
source .local/env.sh                       # Rust/Node/MinGW 全在 D:/AI/.workbuddy/MusicTools/.tools
npm run typecheck && npm run build          # 前端
cargo test -p melodex-core --features net   # 期望 175 passed, 2 ignored
cargo test -p melodex-core --no-default-features   # 期望 89 passed
```

要跑桌面壳的编译检查时**必须**把临时目录指到仓库内，否则 ring 的 C 构建会因 `%TEMP%` 权限失败：

```bash
export TMP=D:/AI/dev/Melodex-Music/.local/tmp TEMP=D:/AI/dev/Melodex-Music/.local/tmp
cargo check --manifest-path src-tauri/Cargo.toml
```

浏览器里看界面（不需要 Rust）：`npm run dev -- --port 5183` → 演示模式（12 首假数据 + 模拟下载）。
`npm run tauri dev` 才是真桌面应用。

---

## 3. 现状快照

- **测试**：core `175 passed, 2 ignored`（其中 1 条真实网络探针 `live_search_then_resolve`、1 条 B 站探针）；
  `--no-default-features` 89 passed；fmt / clippy `-D warnings` 干净；`src-tauri cargo check` 零告警。
- **规模**：core 35 个 `.rs`、前端 24 个 `.vue`、i18n 词条 570+ 条（中英一一对应）。
- **CI**：`.github/workflows/ci.yml`（fmt + clippy + 两套 test + typecheck/build）、
  `release.yml`（windows / android / linux / macos 四个 job + publish，约 20 分钟）、
  `bundle-check.yml`（只在 `bundle-check/**` 分支跑 Windows 打包，专治配置类问题）。
- **文档**：`README.md`（中文）与 `README.en.md`（英文，互为语言切换）、`CHANGELOG.md`（每版都有详细段）、
  `docs/github-listing.md`（GitHub 简介/Topics 文案）。

---

## 4. 架构地图

```
crates/melodex-core/src/
  platform.rs        Registry：四个音源注册
  platforms/         netease.rs qq.rs kuwo.rs kugou.rs shared.rs(公共解析) stream.rs(取流头)
  aggregator.rs      同曲合并 + 相关性重排 + 版本标记分组
  model.rs           Track/Variant/Source/Quality，VersionClass 与 version_tag()
  quality.rs         四平台各自的档位表
  net.rs             客户端池：按"用途 × 音源"选代理（ProxyFor / ProxyScopes / set_sources）
  download/          engine.rs(队列/并发/降级/重签) transfer.rs(断点续传/限速) tag.rs(ID3v2.3)
  login/             netease.rs qq.rs（扫码）；Kuwo/Kugou 走 Tauri 内置登录窗口
  settings.rs        持久化设置（含 proxy / proxyScopes / proxySources）
  update.rs          GitHub Release 检查 + 资产匹配
  local_audio.rs     本地音频识别与命令行参数解析
  bilibili.rs        B 站音乐区搜索（buvid 风控处理）
src-tauri/src/lib.rs 命令层：save_text_file / pick_audio_files / startup_audio_files / bilibili_search /
                     start_web_login / watch_web_login / login_qr_* / open_url / reveal_path
src/
  views/     SearchView RecentView DownloadsView LibraryView PlaylistsView SettingsView PlatformsView
  components/ PlayerBar LyricView QueuePanel TrackRow TrackMenu DownloadPicker AccountPanel AccountLogin
              OnboardingDialog UpdateNotice ToastStack TabBar SideBar BrandMark
  stores/    search tasks accounts library player settings ui onboarding trackMenu
  lib/       i18n.ts format.ts origin.ts reason.ts streaming.ts media-session.ts history.ts types.ts
  i18n/      zh.ts en.ts
```

关键实现要点（改动前先知道）：

- **合并**：`normalize_title` 会去掉括号，所以版本区分靠 `model.rs` 的 `VERSION_MARKS` / `version_tag()`
  （伴奏、纯音乐、Live、Remix、翻唱各自成条；只认标题里真实出现的标记，不猜）。
- **代理**：`Net` 同时按「用途」（搜索/取链/下载/更新）和「音源」放行；平台代码统一用 `text_src/json_src(SOURCE, …)`。
- **取流头**：每个平台声明 `stream_headers()`，播放（本地回环代理）和下载共用同一份，所以补 Referer 一次就两边都对。
- **酷狗**：128k / 320k / 无损是三个不同 hash，存在 `Variant.alt_ids`（`sq:` / `hq:` 前缀），取链用 `m.kugou.com/app/i/getSongInfo.php`（免签名）。
- **i18n**：`t(key, params?)` 点号分级 + `{name}` 占位，缺词条回落中文再回落 key；常量表（主题、音质、UI 风格、歌词主题）存键不存中文。

---

## 5. 已完成（按版本）

| 版本 | 内容 |
| --- | --- |
| v1.6.0 / 1.6.1 | 单任务限速（滑块 + 自定义输入，≤1GB/s） |
| v1.7.0 / 1.7.1 | 全局代理（协议白名单、运行时生效）、歌单导出/导入（JSON 带完整变体） |
| v1.8.0 | 代理按「用途」分流、酷我 Cookie 登录、原唱只认平台官方标记、播放队列面板、搜索历史 + 最近播放 |
| v1.9.0 | 右键菜单、播放模式（顺序/单曲/随机）、音量百分比 + 滚轮、"最近"页签、搜索历史下拉、QQ 下载 403 修复（补 Referer + 403 重取链） |
| v2.0.0 | 伴奏/原唱判重修复、macOS + Linux 打包、新图标（紫罗兰三杠）、下载选择框可用性（响应式 + 可用性标注 + 摘要 + toast） |
| v2.0.1 | Android 更新包安装授权（`ClipData` + `grantUriPermission` + `<queries>`）、macOS 改通用 dmg |
| v2.1.0 | 见下（19 个提交的大版本） |
| v2.1.1 | 文件关联改 ASCII 声明，双击音频用 Melodex 打开 |

**v2.1.0 明细**

1. 缺陷：网易云歌单读 `trackIds` 拉全量（500/批 + `order_by_ids` 还原顺序）；歌单页批量音质下拉；
   曲库独立成 `melodex.library.v1`（清下载记录不再清曲库）；点歌手名即搜；下载框提示「曲库里已有这首」；
   凭据过期 → 账号卡片显示「登录已过期 · 请重新登录」+ 一键重登（`melodex.stale.v1`）。
2. 双语：`src/lib/i18n.ts` + `zh.ts`/`en.ts`，570+ 键；`README.en.md` 全量英文版。
3. 歌词页顶部歌名/歌手居中（`.lv-head` 改 grid）。
4. 系统媒体会话（`src/lib/media-session.ts`）：锁屏/通知栏/厂商胶囊走系统通道。
5. 本地音频：`local_audio.rs` + `pick_audio_files` / `startup_audio_files` 命令；v2.1.1 补齐安装包文件关联。
6. 平台：酷狗接入（真能搜能取流）；「更多平台」页（Apple Music 搜索+30 秒试听+跳转；Spotify/YouTube 只跳转；B 站音乐区能搜）；
   代理按音源；酷我登录弹窗写明"没有扫码"的原因。
7. 重构：`platforms/shared.rs` 抽公共解析；24 小时合规声明进首启引导最后一页与设置页。

**验证方式**（这套流程好用，接着用）：`typecheck/build` + 两套 cargo test + `src-tauri cargo check` +
用 `.local/tools/shot.mjs` 在演示模式里截图/取 EVAL 值核对界面状态 + 真实接口用 `#[ignore]` 探针。

---

## 6. 还没做 / 已知问题

按"值不值得做"排序。前三条是真需求，后面是能力边界。

1. **酷我歌词**（接口要 CSRF 票据，未接）——`platforms/kuwo.rs` 里加 `lyric()`，拿 `csrf` + cookie 试 `mobi.s?f=web&…lyric`。
2. **酷狗歌词**（接口要签名，未接）——`platforms/kugou.rs` 现只有 search/resolve；歌词接口需 `signature=md5(md5(hash)+"kgcloud")`。
3. **QQ 登录保活**：现在只做到"过期能发现、能一键重登"，没有主动续期。可以定时校验 cookie 或按失败次数提前提示。
4. **v2.1.1 的文件关联没在真机装包验证过**——只验证了 Windows 打包能过（`bundle-check.yml` 两次 success）。
5. **Rust 侧错误文案仍是中文硬编码**（`error.rs` 的 `reason()`、部分 `format!`），前端双语了但内核报错没有；
   要做得把 `Reason` 映射到词条，而不是把字符串传给前端。
6. **注释标准化只做了一部分**（新文件按规范写，旧文件还混着口语）。
7. **Spotify / YouTube Music / Apple Music 全曲**：DRM（Widevine / FairPlay），不做也不该做，详见第 7 节。
8. **B 站播放**：DASH 分离流 + 绑 Referer，浏览器 `<audio>` 发不出该头，现只做搜索+跳转。
9. **酷我扫码**：官方没有公开接口（客户端二维码是私有协议），已确认不可行，UI 上已写明。
10. **移动端下载暂停/恢复的真机验证**没做（演示模式覆盖不到系统杀进程）。

---

## 7. 能力边界（别向用户承诺）

- **不做 DRM 破解**：Apple Music 全曲 FairPlay、Spotify 全曲 Widevine，无明文直链。能做的只有：搜索 + 30 秒试听（Apple 的 `previewUrl` 是明文 m4a）+ 跳转。
- **不内置账号/cookie/逐流破解**：一切凭据来自用户自己的登录（`CredStore`）。
- **安卓"灵动岛"没有公开 API**：国内厂商的岛上展示是系统媒体通知的样式化，只能通过 MediaSession 接入，不能自绘岛上 UI。
- **酷我无扫码**、**B 站音频不代播**——都在 UI 上写清了，不要再当 bug 修。
- 合规口径（用户要求）：不提供、不分发、不存储；仅个人试听；24 小时内删除；不得传播商用。任何新功能都不能越过这条线。

---

## 8. 用户口味与既定决策（别推翻）

- 用户是产品拥有者，中文沟通；喜欢"先摸清接口再动手"，讨厌假按钮与假数据 → **做不到的事要在界面上写明白**，不要放点了没反应的入口。
- 原唱/翻唱标记**只认平台官方判定**（网易云 `originCoverType`），QQ/酷我不猜标题。
- 平台扩展优先级：国内四源为主（网易云/QQ/酷我/酷狗），境外平台只做"能用的部分"。
- 代理默认全部音源走代理、默认全部用途开启（老配置兼容），国内源建议用户自己勾掉。
- 版本号跟语义化：修 bug 出补丁版（2.1.x），功能批出小版本（2.x.0）。
- 发版要有**详细**中文 CHANGELOG（用户明确要求"写详细更新日志"）。

---

## 9. 环境陷阱（每条都踩过）

1. `source .local/env.sh` 后 PATH 必须是 `/d/AI/...`（MSYS 风格）；`RUSTUP_HOME`/`CARGO_HOME` 反过来要用 `D:/AI/...`。
2. 仓库路径不能带中文（MinGW 的 `ld.exe` 是 ANSI 版，会报 `cannot find xxx.o` 但文件明明在）。
3. `src-tauri` 构建必须设 `TMP`/`TEMP` 到仓库内 `.local/tmp`，否则 ring 的 C 构建报 `Cannot create temporary file`。
4. **`bundle.fileAssociations` 的 `name`/`description` 只能 ASCII**：写中文 → Windows NSIS/MSI 打包直接失败，
   Linux/macOS/Android 却全绿，极易误判成偶发。每个扩展名单独一条。
5. Vite HMR 会残留旧报错遮罩，改完 dev 侧文件没生效就重启 dev server。
6. 演示模式的 `__APP_VERSION__` 来自 `package.json`，改版本后必须重启 dev server。
7. 截图/自动化用 `.local/tools/shot.mjs`：`node shot.mjs <url> <w> <h> <dpr> <out> <waitMs> [script]`，
   末参脚本返回值会打成 `EVAL:`；临时 profile 不跨运行持久，seed localStorage 后必须同一次运行内截图。
8. 关闭首启引导用 `button.close` / `.mask .close`；整页 reload 会白屏，SPA 内 `location.hash` 跳转最稳。
9. 录屏工具 `.local/tools/record.mjs`（CDP screencast）：`--window-size` 必须 `W, H+150`，别混用 `Emulation.setDeviceMetricsOverride`。
10. Actions 日志需要 token（现在过期）；Release 资产直链被墙，核对产物只用 `api.github.com`。

---

## 10. 代码约定

- Rust：`rustfmt` 默认风格；`clippy -D warnings` 必须过；错误统一走 `Error`（`error.rs`），带 `Reason` 枚举。
- 动 `Source` 枚举（`model.rs`）必须同时 `cargo check --manifest-path src-tauri/Cargo.toml`——外壳里有 5 处 `match source`。
- 前端：Vue 3 `<script setup>` + Pinia store；组件里不许直接写中文，一律 `t('key')`，词条加进 `zh.ts` 与 `en.ts` 两份。
- 测试：解析类逻辑写成纯函数 + 单测；真实网络调用放 `#[ignore]` 探针，本地手动跑 `-- --ignored --nocapture`。
- 提交信息用 `feat(scope): …` / `fix(scope): …` / `chore(release): x.y.z — …`，正文写清为什么。

---

## 11. 发版流程（照抄即可）

```bash
# 1) 改版本号四处 + 刷 lock
#    package.json / src-tauri/tauri.conf.json / Cargo.toml:7 / src-tauri/Cargo.toml:3
cargo check -p melodex-core --features net        # 更新 Cargo.lock 里的版本

# 2) 写 CHANGELOG 段（必须有：修复/新增/改动，带原因）

# 3) 全量校验
cargo fmt --check
cargo clippy -p melodex-core --features net --all-targets -- -D warnings
cargo test -p melodex-core --features net && cargo test -p melodex-core --no-default-features
npm run typecheck && npm run build
export TMP=$PWD/.local/tmp TEMP=$PWD/.local/tmp && cargo check --manifest-path src-tauri/Cargo.toml

# 4) 提交 + 推 + 打 tag
git commit -am "chore(release): x.y.z — …" && git push origin main
git tag -a vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z

# 5) 等 release workflow（约 20 分钟），用匿名 API 核对
curl -s https://api.github.com/repos/LunaShiori/Melodex-Music/releases/tags/vX.Y.Z
```

打包配置类改动（bundle、文件关联、产物名）**先用** `bundle-check.yml`：建个 `bundle-check/xxx` 分支 push，
等 10 分钟看 conclusion，比整轮 release 便宜得多。release 失败时重发要先删远端 tag 再重打。

---

## 12. 常用命令速查

```bash
source .local/env.sh                                   # 环境
npm run dev -- --port 5183                             # 演示模式
npm run tauri dev                                      # 桌面开发
npm run tauri build -- --bundles nsis,msi              # Windows 包
node .local/tools/shot.mjs "http://localhost:5183/#/search" 1280 860 shot.png 4000 1 'JS脚本'
cargo test -p melodex-core --features net -- --ignored --nocapture   # 真实网络探针
python .local/tmp/check_keys.py                        # i18n 键对齐检查（脚本在 .local）
```

`.local/` 下还有：`memory/`（工程日志）、`shots/`（各版本截图）、`video/`（演示视频源工程与成片）、
`tools/`（截图/录屏工具）、`tmp/`（一次性脚本）。这些都被 git 排除，换机要单独拷。

---

## 13. 建议的上手顺序

1. 第 1 小时：跑第 2 节那几条命令，确认 175 / 89 passed，起演示模式点一遍搜索→下载→曲库→设置。
2. 第 2 小时：读 `model.rs`（Track/Variant/Quality/Source）与 `aggregator.rs`（合并规则）——这是全项目的语义地基。
3. 第 3 小时：读 `net.rs` 的 `pick()` 与 `download/engine.rs` 的主循环——这两处最容易改出 bug。
4. 然后从第 6 节挑一条（建议先做酷我歌词，最小闭环：`platforms/kuwo.rs` + 一条 `#[ignore]` 探针 + 前端无需改）。
5. 每轮结束前：`fmt + clippy + 两套 test + typecheck/build`，截图走查，写 CHANGELOG 草稿，再汇报给用户。

---

## 14. 我踩过、你可能会再踩的坑

- 批量替换模板时用正则容易把 `'标题'` 变成 `'{{ t(...) }}'`（模板语法进了 JS 字符串）；属性插值必须写 `:attr="t('k')"` 而不是 `attr="{{ t('k') }}"`。
- `auto_keys.py` 这类脚本会把 `:placeholder="a ? 'x' : 'y'"` 误改成 `::placeholder`，产出双冒号。
- i18n 常量表里存"翻译后的中文"会导致切语言不生效——要存键，模板里 `t(item.label)`。
- 加 `Source` 枚举后忘了改 `src-tauri` 的 `match` → E0004，CI 核心 job 不查外壳，要单独 check。
- NSIS/WiX 对非 ASCII 的关联描述零容忍（v2.1.0 两次发版失败就是它）。
- 演示模式下载约 300KB/s，10MB 任务要 30 秒；截图前等太短会误判成"没生效"。
