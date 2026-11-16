# 平台 Beta 设计与开源调研

- Meting https://github.com/metowolf/Meting ：参考多音源适配器统一接口的设计；现有 Rust 核心已具备此结构，未引入额外 Node 服务。
- LX Music Desktop https://github.com/lyswhut/lx-music-desktop ：参考平台能力分离与用户可选来源思路；没有复制源码。
- Nemo2011/bilibili-api https://github.com/Nemo2011/bilibili-api ：检查时仓库已归档，不作为新的运行时依赖。

本轮为原创实现，无新增第三方运行依赖。主搜索可选择同时检索 Beta，结果按来源分区，保留各平台独立错误和加载状态。Apple Music 提供片段试听及来源跳转；B 站提供视频搜索与跳转；Spotify / YouTube Music 提供官网搜索入口。

外部平台故障不影响国内四源结果。Beta 不进入全曲下载队列；不把预览片段标记为全曲。试听与主播放器互斥；外链交给系统浏览器。后续接入需逐项验证搜索、账号权限、播放与移动端兼容性。
