#!/usr/bin/env python3
"""把 APK 安装插件注入 Tauri 生成的 Android 工程。

为什么需要：`tauri android init` 生成的是一个「干净」工程——没有 FileProvider，
也没有 `REQUEST_INSTALL_PACKAGES` 权限。而 Android 从 7.0 起禁止把 `file://` 交给别的应用
（FileUriExposedException），从 8.0 起还必须显式声明安装权限，否则点安装只会静默失败。

所以 CI 在 init 之后、build 之前调用本脚本，做四件幂等的事：
  1. 把 `src-tauri/android/ApkInstallPlugin.kt` 拷进 app 模块的源码目录（按 identifier 改 package）
  2. 写入 `res/xml/file_paths.xml`（FileProvider 的可见范围）
  3. 给 AndroidManifest 补 `REQUEST_INSTALL_PACKAGES` 权限与 FileProvider 声明
  4. 确认 `androidx.core` 依赖在（FileProvider 就在这个包里）

为什么要 fail-fast：Rust 侧 `register_android_plugin` 是运行时按类名反射实例化的。
如果脚本悄悄跳过，插件注册会失败、安装按钮变哑巴——不如直接让构建挂掉。

用法：
    python3 scripts/android-apk-plugin.py [--project src-tauri/gen/android]
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SOURCE_DIR = REPO / "src-tauri" / "android"

USES_PERMISSION = (
    '    <!-- 装自己下载的 APK 需要这条；缺了系统只在后台静默拒绝，不给任何提示 -->\n'
    '    <uses-permission android:name="android.permission.REQUEST_INSTALL_PACKAGES" />\n'
)

PROVIDER = """        <!-- file:// 从 Android 7 起不能跨应用传递，更新包要走 content:// -->
        <provider
            android:name="androidx.core.content.FileProvider"
            android:authorities="${applicationId}.apkprovider"
            android:exported="false"
            android:grantUriPermissions="true">
            <meta-data
                android:name="android.support.FILE_PROVIDER_PATHS"
                android:resource="@xml/file_paths" />
        </provider>
"""

GRADLE_DEP = '    implementation("androidx.core:core-ktx:1.13.1")\n'


def identifier() -> str:
    conf = json.loads((REPO / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    return conf["identifier"]


def patch_manifest(path: pathlib.Path) -> None:
    text = path.read_text(encoding="utf-8")
    original = text

    if "REQUEST_INSTALL_PACKAGES" not in text:
        match = re.search(r"<manifest[^>]*>\s*\n", text)
        if not match:
            raise SystemExit(f"错误：{path} 里找不到 <manifest> 开标签，无法注入权限")
        text = text[: match.end()] + USES_PERMISSION + text[match.end():]

    if "apkprovider" not in text:
        index = text.rfind("</application>")
        if index < 0:
            raise SystemExit(f"错误：{path} 里找不到 </application>，无法注入 FileProvider")
        # 退到这一行的行首再插入，否则 </application> 的缩进会跑到 provider 前面去
        line_start = text.rfind("\n", 0, index) + 1
        text = text[:line_start] + PROVIDER + text[line_start:]

    if text != original:
        path.write_text(text, encoding="utf-8")
        print(f"  补丁 AndroidManifest：{path}")
    else:
        print("  AndroidManifest 已是打过补丁的状态，跳过")


def patch_gradle(path: pathlib.Path) -> None:
    if not path.exists():
        raise SystemExit(f"错误：找不到 {path}，请先执行 `tauri android init`")
    text = path.read_text(encoding="utf-8")
    if "androidx.core" in text:
        print("  build.gradle.kts 已有 androidx.core，跳过")
        return
    index = text.find("dependencies {")
    if index < 0:
        raise SystemExit(f"错误：{path} 里找不到 dependencies 块")
    brace = text.index("{", index)
    text = text[: brace + 1] + "\n" + GRADLE_DEP + text[brace + 1:]
    path.write_text(text, encoding="utf-8")
    print(f"  补依赖 androidx.core：{path}")


def patch(project: pathlib.Path) -> int:
    main = project / "app" / "src" / "main"
    manifest = main / "AndroidManifest.xml"
    if not manifest.exists():
        print(f"错误：找不到 {manifest}，请先执行 `tauri android init`", file=sys.stderr)
        return 1

    pkg = identifier()
    package_dir = main / "java" / pathlib.Path(*pkg.split("."))
    package_dir.mkdir(parents=True, exist_ok=True)

    kotlin_src = (SOURCE_DIR / "ApkInstallPlugin.kt").read_text(encoding="utf-8")
    # package 行跟着 identifier 走，免得改一次标识就编译不过
    kotlin_src = re.sub(r"^package\s+\S+$", f"package {pkg}", kotlin_src, count=1, flags=re.M)
    kotlin_dst = package_dir / "ApkInstallPlugin.kt"
    kotlin_dst.write_text(kotlin_src, encoding="utf-8")
    print(f"  写入 Kotlin 插件：{kotlin_dst}")

    xml_dir = main / "res" / "xml"
    xml_dir.mkdir(parents=True, exist_ok=True)
    (xml_dir / "file_paths.xml").write_text(
        (SOURCE_DIR / "file_paths.xml").read_text(encoding="utf-8"), encoding="utf-8"
    )
    print(f"  写入 FileProvider 路径表：{xml_dir / 'file_paths.xml'}")

    patch_manifest(manifest)
    patch_gradle(project / "app" / "build.gradle.kts")

    # 自检：四样东西都在才算注入成功
    checks = {
        "Kotlin 类": kotlin_dst.exists(),
        "file_paths.xml": (xml_dir / "file_paths.xml").exists(),
        "安装权限": "REQUEST_INSTALL_PACKAGES" in manifest.read_text(encoding="utf-8"),
        "FileProvider": "apkprovider" in manifest.read_text(encoding="utf-8"),
    }
    missing = [name for name, ok in checks.items() if not ok]
    if missing:
        print(f"错误：注入不完整，缺少 {', '.join(missing)}", file=sys.stderr)
        return 1
    print("APK 安装插件注入完成")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="给 Android 工程注入 APK 安装插件")
    parser.add_argument(
        "--project",
        default=str(REPO / "src-tauri" / "gen" / "android"),
        help="Tauri 生成的 Android 工程目录",
    )
    args = parser.parse_args()
    return patch(pathlib.Path(args.project))


if __name__ == "__main__":
    raise SystemExit(main())
