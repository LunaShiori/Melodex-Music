# 平台 Beta 设计与开源调研

- Meting https://github.com/metowolf/Meting ：参考多音源适配器统一接口的设计；现有 Rust 核心已具备此结构，未引入额外 Node 服务。
- LX Music Desktop https://github.com/lyswhut/lx-music-desktop ：参考平台能力分离与用户可选来源思路；没有复制源码。
- Nemo2011/bilibili-api https://github.com/Nemo2011/bilibili-api ：检查时仓库已归档，不作为新的运行时依赖。

本轮为原创实现，无新增第三方运行依赖。主搜索可选择同时检索 Beta，结果按来源分区，保留各平台独立错误和加载状态。Apple Music 提供片段试听及来源跳转；B 站提供视频搜索与跳转；Spotify / YouTube Music 提供官网搜索入口。

外部平台故障不影响国内四源结果。Beta 不进入全曲下载队列；不把预览片段标记为全曲。试听与主播放器互斥；外链交给系统浏览器。后续接入需逐项验证搜索、账号权限、播放与移动端兼容性。

## v2.4.0 实施记录

网易云歌手、专辑资料已进入主搜索的曲目导航；其他平台资料页尚未接入。B 站修复状态注入并加入分页。

酷狗扫码参考公开协议资料 https://github.com/MakcRe/KuGouMusicApi 的 login_qr_key / login_qr_check 模块，独立编写 Rust 会话、校验与解析器；未引入其服务、账号池、风险校验模拟或绕过代码。MD5 请求签名使用 RustCrypto md-5（MIT OR Apache-2.0），仅用于平台协议，账号存储仍使用现有加密保险库。实测二维码申请和等待轮询，真实账号确认仍标 BETA。

待完成：酷我可靠扫码流程；QQ／酷我／酷狗资料页；Apple Music MusicKit 与 Google OAuth 应用配置后的正常授权和歌单接入；各平台按其官方许可支持的下载能力。当前没有这些开发者配置，不提供虚假的授权成功或全曲下载按钮。
