#!/usr/bin/env python3
"""把 src-tauri/icons/android 的启动图标强制同步进 gen/android 工程。

为什么需要这一步：`tauri android init` 每次都会生成一份带 **官方默认图标**
（黄青色图案）的全新工程，构建时并不会自动搬运 `src-tauri/icons/android/`。
CI 上每次都是全新 init，所以打出来的 APK 图标永远是默认的——除非在 init
之后把我们的图标覆盖进去。

用法：python3 scripts/android-icons.py（在仓库根目录执行）
幂等：重复执行只是重复覆盖，无副作用；末尾自检，失败即退出码 1。
"""

import filecmp
import os
import shutil
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "src-tauri", "icons", "android")
DST = os.path.join(ROOT, "src-tauri", "gen", "android", "app", "src", "main", "res")

# 自检用的关键文件：三样齐了启动图标才会走我们的自适应图标
CHECK_FILES = [
    "mipmap-xxxhdpi/ic_launcher.png",
    "mipmap-xxxhdpi/ic_launcher_foreground.png",
    "mipmap-xxxhdpi/ic_launcher_round.png",
    "mipmap-anydpi-v26/ic_launcher.xml",
    "values/ic_launcher_background.xml",
]


def main() -> int:
    if not os.path.isdir(SRC):
        print(f"[icons] 源目录不存在：{SRC}", file=sys.stderr)
        return 1
    if not os.path.isdir(DST):
        print(f"[icons] gen 工程不存在：{DST}（请先运行 tauri android init）", file=sys.stderr)
        return 1

    copied = 0
    for dirpath, _dirnames, filenames in os.walk(SRC):
        for name in filenames:
            src_file = os.path.join(dirpath, name)
            rel = os.path.relpath(src_file, SRC)
            dst_file = os.path.join(DST, rel)
            os.makedirs(os.path.dirname(dst_file), exist_ok=True)
            shutil.copyfile(src_file, dst_file)
            copied += 1
    print(f"[icons] 已覆盖 {copied} 个图标资源")

    # 自检：逐个字节比对，任何一个对不上都直接失败
    ok = True
    for rel in CHECK_FILES:
        src_file = os.path.join(SRC, rel)
        dst_file = os.path.join(DST, rel)
        if not os.path.isfile(dst_file):
            print(f"[icons] 缺失 {rel}", file=sys.stderr)
            ok = False
        elif not filecmp.cmp(src_file, dst_file, shallow=False):
            print(f"[icons] 内容不一致 {rel}", file=sys.stderr)
            ok = False
    if not ok:
        print("[icons] 自检失败", file=sys.stderr)
        return 1
    print("[icons] 自检通过：APK 将使用与桌面一致的图标")
    return 0


if __name__ == "__main__":
    sys.exit(main())
