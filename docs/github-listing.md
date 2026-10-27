# GitHub 仓库展示配置（简介 / Topics / 社交预览）

> 这份文件是**可直接照抄**的仓库门面配置。GitHub 的搜索排序主要看：仓库名 → 简介（description）→
> Topics → README 正文关键词 → star / 活跃度。下面每段都用改好。

## 0. 一条命令搞定（推荐）

网页上逐项点太慢，仓库里有个脚本能一次做完三件事：写简介、写 20 个 Topics、
把 v0.1.0 标记成 pre-release（那个版本有「点下载即崩溃」的 bug，别让新用户下到）。

只差一个令牌，创建流程 5 步、约 1 分钟：

1. 打开 <https://github.com/settings/tokens>（头像 → Settings → 左下角 Developer settings →
   Personal access tokens → **Tokens (classic)**）；
2. 右上 **Generate new token** → **Generate new token (classic)**；
   可能会要求输入密码或走一次 2FA，正常验证即可；
3. Note 填 `melodex-repo-meta`，Expiration 选 **7 days**（用完就删，不必给长有效期）；
4. 勾选权限：只勾最上面的 **`repo`** 整个大类（它会连带勾上下面几项，这是正常的）。
   不要勾 `delete_repo`、`admin:org`、`workflow` 这些用不到的；
5. 页面拉到底点 **Generate token**，出来的 `ghp_` 开头字符串**只显示这一次**，立刻复制。

然后在本仓库根目录：

```bash
export GH_TOKEN=ghp_刚才复制的一串       # 注意：等号两边不要空格
bash scripts/setup-github-meta.sh
```

正常会输出三行 `[OK]`。看到 `[FAIL] ... 403` 就是权限没给够，回到第 4 步重勾。

**用完立刻撤销**：<https://github.com/settings/tokens> → 找到 `melodex-repo-meta` → Delete。
令牌等同于密码，泄漏后别人能以你的身份改仓库。

> 为什么不用 fine-grained token：它按仓库授权，改仓库简介属于 “Administration”，
> 勾选面板藏得深、容易漏勾（本项目实测就撞过 403）。classic 勾一个 `repo` 就够了。
>
> 另外：不要用 `git config` 里已存的推送令牌做这件事——那把通常是只读或仅有 Contents 权限，
> 写简介会返回 `Resource not accessible by personal access token`。

改完后刷新仓库首页即可看到效果；GitHub 的搜索索引大约需要几分钟到几小时才更新。

<details>
<summary>不想跑脚本？等价的三条 curl</summary>

```bash
export GH_TOKEN=ghp_你的令牌

curl -sS -X PATCH https://api.github.com/repos/LunaShiori/Melodex-Music \
  -H "Authorization: Bearer $GH_TOKEN" -H "Accept: application/vnd.github+json" \
  -d '{"description":"Melodex Music｜多源音乐聚合搜索、播放与歌词（Windows + Android）…"}'

curl -sS -X PUT https://api.github.com/repos/LunaShiori/Melodex-Music/topics \
  -H "Authorization: Bearer $GH_TOKEN" -H "Accept: application/vnd.github+json" \
  -d '{"names":["music","music-player","lyrics","music-downloader","music-search","music-aggregator","music-library","netease-cloud-music","qqmusic","kuwo","lrc-lyrics","id3v2","tauri","tauri-app","rust","vue3","typescript","android","windows","open-source"]}'

# 先把 v0.1.0 的 release id 查出来，再改
RID=$(curl -sS -H "Authorization: Bearer $GH_TOKEN" \
  https://api.github.com/repos/LunaShiori/Melodex-Music/releases/tags/v0.1.0 \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
curl -sS -X PATCH "https://api.github.com/repos/LunaShiori/Melodex-Music/releases/$RID" \
  -H "Authorization: Bearer $GH_TOKEN" -H "Accept: application/vnd.github+json" \
  -d '{"prerelease":true}'
```

</details>

## 1. 简介（About → Description，上限 350 字符）

**推荐版（中文优先，尾部带英文检索词）**

```
Melodex Music｜多源音乐聚合搜索、播放与歌词（Windows + Android）。一个关键词同时搜网易云音乐、QQ音乐、酷我音乐，按「标题+歌手+时长」指纹合并同一首歌并标注每个音源是否可播；内置播放器与逐行歌词（含双语翻译），支持账号歌单、置顶原唱、断点续传、音质自动降级、ID3v2.3 标签与歌词内嵌。Rust + Tauri 2 + Vue 3，Apache-2.0。music player / lyrics / music downloader / music aggregator
```

**英文优先版（想让海外用户先搜到就用这版）**

```
Melodex Music — cross-source music search, playback and lyrics for Windows and Android. Search NetEase Cloud Music, QQ Music and Kuwo in one query, merge duplicates by title/artist/duration fingerprint, and see per-source availability. Built-in player with line-synced bilingual lyrics, account playlists, resumable downloads, automatic bitrate fallback, ID3v2.3 tags with embedded cover & lyrics. Rust, Tauri 2, Vue 3. Apache-2.0. 中文：多源音乐聚合搜索、播放与歌词
```

**为什么这么写**

- 前 60 个字符是搜索结果里唯一稳定可见的部分，所以把「产品名 + 是什么 + 支持什么平台」放在最前；
- 同时出现「音乐」「搜索」「下载」「网易云」「QQ音乐」「酷我音乐」这些用户真实会输入的词，
  而不是只写「强大的工具」这类无人检索的形容词；
- 中文用户搜「音乐下载器」「音乐聚合」，英文用户搜 `music downloader`、`music aggregator`，
  两种词都覆盖到，避免只有一边能被命中；
- 结尾带技术栈（Rust / Tauri / Vue）能吃到「Tauri 应用」这类开发者检索流量。

## 2. Topics（仓库右上角齿轮 → Topics，最多 20 个，全部小写、用连字符）

```
music
music-downloader
music-search
music-aggregator
netease-cloud-music
qqmusic
kuwo
tauri
tauri-app
rust
vue3
typescript
android
windows
cross-platform
desktop-app
id3v2
lrc-lyrics
resumable-download
open-source
```

粘贴方式：在 Topics 输入框里逐个回车即可。前 5 个权重最高，所以把最核心的
`music` / `music-downloader` / `music-search` 放在最前。

## 3. README 首屏（已优化，勿删这几个词）

仓库根目录 `README.md` 的前 30 行刻意包含了搜索会命中的关键词组合：

- 标题行：`多源音乐聚合搜索与下载 · Windows 桌面端 + Android 手机端`
- 一句话简介里出现：网易云、QQ音乐、酷我音乐、聚合搜索、批量下载、断点续传、音质降级、ID3、歌词
- 「常见问题」小节里回答了「能下载哪些平台的歌」「和同类工具的区别」——这类长尾问题正是
  用户搜索时的自然语言，GitHub 会索引 README 正文。

## 4. 社交预览图（Settings → Social preview）

上传一张 1280×640 的图，内容建议：

- 左侧：`docs/screenshots/desktop-search.png` 的搜索结果区截图（能一眼看出是多源结果）
- 右侧：产品名 Melodex Music + 一句 `多源音乐聚合搜索、播放与歌词 · Windows / Android`
- 底色用品牌紫 `#6b5bf0` 的深色渐变

这张图会出现在 Twitter / 微信 / Telegram 分享卡片里，是「别人愿意点进来」的关键。

## 5. Release 的标题与摘要（让搜「Melodex 下载」的人落到这里）

标题：`Melodex Music 0.1.0 · Windows 安装包 + Android APK`（`release.yml` 里已按这个格式自动生成）

摘要首段固定写清「是什么 + 装哪个文件 + 怎么升级」，例如：

```
多源音乐聚合搜索与下载器，本次为首个可安装版本。
Windows 用户下载 MelodexMusic_0.1.0_x64-setup.exe 双击安装；
Android 用户下载 MelodexMusic_0.1.0_universal.apk（允许「安装未知来源应用」）。
装好后再发新版，应用内「设置 → 关于 → 检查更新」会自动提示。
```

## 6. 仓库改名记录（v1.3.0 已完成）

GitHub 的仓库名权重高于简介。原来叫 `melodex`，是纯品牌词，搜「音乐播放器」「音乐下载器」都不会命中。
v1.3.0 改成了 `Melodex-Music`：

- **为什么带连字符**：GitHub 仓库名不允许空格，`Melodex-Music` 会被分词成 `melodex` + `music` 两个词，
  搜索命中率高于连写的 `MelodexMusic`。
- **改了什么**：`Cargo.toml` / `package.json` 的 `repository` 与 `homepage`、
  `crates/melodex-core/src/update.rs` 的 `DEFAULT_FEED`、README 里的克隆地址与本页所有 curl 示例。
- **没改什么**：`identifier`（`app.melodex`）与 `Cargo.toml` 包名。前者决定用户数据目录与 Android 包名，
  改了等于换一个应用；后者只是构建产物名，与可发现性无关。
- **旧链接**：GitHub 会给旧仓库名留 301 跳转，历史下载链接与 `git clone` 旧地址都还能用，
  但本地 `git remote` 建议执行一次
  `git remote set-url origin git@github.com:LunaShiori/Melodex-Music.git`。
- **改名方式**：网页 `Settings → General → Repository name`，或跑 `scripts/setup-github-meta.sh`
  （第一步就是改名的幂等操作，需要带 `Administration: RW` 的令牌）。
