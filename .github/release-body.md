## 安装方式

| 平台 | 选哪个文件 | 说明 |
| --- | --- | --- |
| Windows 10/11 x64 | `*_x64-setup.exe` | 推荐，双击安装，含开始菜单与卸载项 |
| Windows 10/11 x64 | `*_x64_en-US.msi` | 企业分发 / 静默安装 |
| macOS 11+ Apple 芯片 | `*_aarch64.dmg` | 拖进「应用程序」即可 |
| macOS 11+ Intel | `*_x86_64.dmg` | 同上 |
| Linux x64 | `*_amd64.deb` | Debian / Ubuntu：`sudo dpkg -i` 后补依赖 |
| Linux x64 | `*_amd64.AppImage` | 免安装，`chmod +x` 后直接运行 |
| Android 8.0+ | `*.apk` | 通用包，需允许「安装未知来源应用」 |

首次安装后，应用会在启动时静默检查更新；有新版本时左下角会出现提示，
也可以在「设置 → 关于 → 检查更新」里手动触发。

> Android 覆盖升级要求签名一致：本项目所有 Release 均由同一把 CI 密钥签名，
> 因此新版本可以直接覆盖安装，无需先卸载。

> macOS 与 Windows 包目前没有代码签名证书：macOS 首次打开需右键「打开」，或在
> 「系统设置 → 隐私与安全性」里点「仍要打开」；Linux 的 AppImage 在部分发行版需要
> `sudo apt install libfuse2`。

**Melodex Music** 是一个多源音乐聚合搜索、播放与下载工具：一个关键词同时检索网易云音乐、
QQ 音乐与酷我音乐，按「标题 + 歌手 + 时长」指纹把同一首歌合并成一条，
逐源标注可用性与码率，内置播放器与逐行歌词，支持断点续传、音质自动降级、
ID3v2.3 标签与歌词内嵌。
Rust 核心 + Tauri 2 + Vue 3，Apache-2.0 开源，仅用于个人学习与技术研究。
