#!/usr/bin/env python3
"""给 Tauri 生成的 Android 工程补上 release 签名配置。

为什么需要：`tauri android init` 生成的 `app/build.gradle.kts` 里既没有 `signingConfigs`，
release buildType 也没有指向任何签名配置，Gradle 只会产出 `app-universal-release-unsigned.apk`。
未签名的 APK 无法安装，所以 CI 在 init 之后、build 之前调用本脚本做幂等补丁。

用法：
    python scripts/android-signing.py [--project src-tauri/gen/android]

脚本会：
  1. 确保文件顶部有 `import java.io.FileInputStream`
  2. 在 `buildTypes {` 之前插入读取 keystore.properties 的 signingConfigs 块
  3. 给 `getByName("release") {` 加上 signingConfig
已打过补丁时直接返回 0，可重复执行。
"""

from __future__ import annotations

import argparse
import pathlib
import sys

SIGNING_BLOCK = """    signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            val keystoreProperties = Properties()
            if (keystorePropertiesFile.exists()) {
                keystoreProperties.load(FileInputStream(keystorePropertiesFile))
            }
            keyAlias = keystoreProperties["keyAlias"] as String
            keyPassword = keystoreProperties["password"] as String
            storeFile = file(keystoreProperties["storeFile"] as String)
            storePassword = keystoreProperties["password"] as String
            // 仓库里提交的是 PKCS#12 密钥；若换成 JKS，在 keystore.properties 里显式写 storeType=JKS
            storeType = (keystoreProperties["storeType"] as String?) ?: "PKCS12"
        }
    }
"""


def patch(gradle: pathlib.Path) -> int:
    if not gradle.exists():
        print(f"错误：找不到 {gradle}，请先执行 `tauri android init`", file=sys.stderr)
        return 1

    text = gradle.read_text(encoding="utf-8")

    if 'create("release")' in text and "signingConfig = signingConfigs" in text:
        print("已存在 release 签名配置，跳过补丁")
        return 0

    changed = False

    if "import java.io.FileInputStream" not in text:
        text = "import java.io.FileInputStream\n" + text
        changed = True

    if 'create("release")' not in text:
        marker = "\n    buildTypes {"
        if marker not in text:
            print("错误：build.gradle.kts 中找不到 `buildTypes {`，模板可能已变更", file=sys.stderr)
            return 1
        text = text.replace(marker, "\n" + SIGNING_BLOCK + marker, 1)
        changed = True

    if "signingConfig = signingConfigs" not in text:
        marker = 'getByName("release") {'
        if marker not in text:
            print('错误：找不到 `getByName("release") {`，模板可能已变更', file=sys.stderr)
            return 1
        text = text.replace(marker, marker + "\n            signingConfig = signingConfigs.getByName(\"release\")", 1)
        changed = True

    if changed:
        gradle.write_text(text, encoding="utf-8")
    print(f"签名配置已写入 {gradle}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="为 Tauri Android 工程注入 release 签名配置")
    parser.add_argument(
        "--project",
        default="src-tauri/gen/android",
        help="android init 生成的工程根目录（默认 src-tauri/gen/android）",
    )
    args = parser.parse_args()
    return patch(pathlib.Path(args.project) / "app" / "build.gradle.kts")


if __name__ == "__main__":
    raise SystemExit(main())
