# 贡献指南

感谢你愿意改进 Melodex。这份文档说明项目的工程约定，让改动更容易被合并。

## 开发环境

| 工具 | 版本 | 说明 |
| --- | --- | --- |
| Node.js | ≥ 18 | 前端构建 |
| Rust | ≥ 1.77 | 核心与外壳 |
| Python | ≥ 3.9 | 仅用于生成图标与第三方许可清单（可选） |
| Android SDK/NDK | API 34 / NDK 26 | 仅在打 Android 包时需要 |
| Visual Studio Build Tools | 2022 | 仅 Windows 桌面端构建需要（C++ 工作负载） |

```bash
npm install
npm run tauri dev        # 桌面端开发
npm run dev              # 纯前端（演示模式，内置假数据，无需 Rust）
```

## 提交前自检

```bash
cargo test -p melodex-core --no-default-features   # 核心纯逻辑测试
cargo check -p melodex-core --features net         # 网络层类型检查
cargo fmt --all
cargo clippy -p melodex-core --features net -- -D warnings
npm run typecheck                                  # vue-tsc
npm run build                                      # 生产构建
```

四条都要过。CI（`.github/workflows/build.yml`）会跑同样的检查。

## 代码分层约定

- **`melodex-core` 不许依赖 Tauri、不许依赖任何 UI 概念。** 新增能力先想清楚它属于核心还是外壳。
- **外壳（`src-tauri`）只做三件事**：参数转换、错误字符串化、事件转发。不要在 command 里写业务判断。
- **界面不直接调 Tauri API**，一律走 `src/lib/transport` 的 `Transport` 接口，并同步实现 `demo.ts` 的演示分支，
  否则浏览器演示模式会挂。
- **新增设置项**要同时改三处：`settings.rs` 的模型与 `sanitized()`、`stores/settings.ts` 的默认值、`SettingsView.vue` 的表单。

## 新增一个音源

1. 在 `crates/melodex-core/src/platforms/` 下新建 `xxx.rs`，实现 `MusicPlatform` 的四个方法；
2. 在 `model.rs` 的 `Source` 枚举里加一项（`id / label / accent / from_id / ALL`）；
3. 在 `Registry::new` 里注册；
4. 在 `src/lib/types.ts` 的 `SOURCE_META` 里补上颜色与短名；
5. 用 `cargo run --example probe -- <歌名>` 验证搜索、取流、歌词三条链路。

请只接入**公开接口**。不要提交任何账号、cookie、token、签名密钥或绕过付费的逻辑。

## 测试要求

- 纯逻辑（指纹合并、LRC 解析、命名清洗、状态迁移、配置归一化）必须有单元测试，放在对应模块的
  `#[cfg(test)] mod tests` 里。
- 涉及网络的代码写「可离线验证」的解析测试：把真实响应裁剪成样本字符串，断言解析结果。
- 不要写会真连外网的测试。

## 提交信息

用一句话说清「改了什么 + 为什么」，推荐 Conventional Commits：

```
feat(kuwo): 支持无损音质档位
fix(engine): 暂停后恢复时重复写进度
docs(readme): 补充 Android 签名说明
```

## 分支与 PR

- 从 `main` 切分支：`feat/<topic>`、`fix/<topic>`。
- 一个 PR 只做一件事；涉及界面改动请附截图（桌面 + 移动各一张）。
- PR 描述里写：动机、方案、影响面、自检结果。

## 许可证

提交贡献即表示你同意以 [Apache-2.0](LICENSE) 授权你的贡献，并保证你有权这样做。
新增第三方依赖时，请同步更新 `THIRD_PARTY_LICENSES.txt`（`python3 scripts/gen_third_party.py -o THIRD_PARTY_LICENSES.txt`）
以及 `scripts/gen_third_party.py` 里的许可证表。

## 行为准则

对事不对人。技术分歧用基准测试和复现步骤说话，不欢迎人身攻击、政治与地域攻击、以及任何形式的骚扰。
