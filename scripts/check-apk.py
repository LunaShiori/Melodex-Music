#!/usr/bin/env python3
"""校验产出的 APK 是否真的被签名（未签名的包装不到手机上）。

为什么不能只看 META-INF：
  Android 7+ 默认使用 APK Signature Scheme v2/v3，签名信息写在 ZIP 中央目录**之前**的
  "APK Signing Block" 里，不再生成 META-INF/*.RSA/*.SF（那是 v1 / JAR 签名的产物）。
  只查 META-INF 会把已签名的 APK 误判为未签名。

判定顺序：
  1. 文件名带 "-unsigned" → 直接失败（签名配置没生效）
  2. 优先调用 apksigner verify（若环境里有 Android build-tools）
  3. 否则自行探测：v2/v3 的 "APK Sig Block 42" 魔数，或 v1 的 META-INF 签名块

用法：
    python scripts/check-apk.py out/*.apk
"""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import zipfile

SIG_BLOCK_MAGIC = b"APK Sig Block 42"


def find_apksigner() -> str | None:
    apksigner = shutil.which("apksigner")
    if apksigner:
        return apksigner

    roots = []
    for env in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        value = os.environ.get(env)
        if value:
            roots.append(pathlib.Path(value))
    roots.append(pathlib.Path("/usr/local/lib/android/sdk"))

    for root in roots:
        build_tools = root / "build-tools"
        if build_tools.is_dir():
            for candidate in sorted(build_tools.glob("*/apksigner"), reverse=True):
                return str(candidate)
    return None


def verify_with_apksigner(apk: pathlib.Path, apksigner: str) -> bool | None:
    try:
        proc = subprocess.run(
            [apksigner, "verify", "--verbose", str(apk)],
            capture_output=True,
            text=True,
            timeout=180,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if proc.returncode == 0:
        schemes = [
            line.strip()
            for line in proc.stdout.splitlines()
            if "scheme" in line.lower() and "true" in line.lower()
        ]
        print(f"  apksigner: OK{'（' + ', '.join(schemes) + '）' if schemes else ''}")
        return True
    print(f"  apksigner: 校验失败\n{proc.stdout}\n{proc.stderr}", file=sys.stderr)
    return False


def detect_manually(apk: pathlib.Path) -> tuple[bool, str]:
    if SIG_BLOCK_MAGIC in apk.read_bytes()[-8 * 1024 * 1024 :]:
        return True, "v2/v3 (APK Signing Block)"

    with zipfile.ZipFile(apk) as zf:
        names = [n.upper() for n in zf.namelist() if n.upper().startswith("META-INF/")]
    if any(n.endswith((".RSA", ".DSA", ".EC")) for n in names) and any(
        n.endswith(".SF") for n in names
    ):
        return True, "v1 (JAR 签名)"

    return False, "未找到任何签名"


def main(argv: list[str]) -> int:
    targets = [pathlib.Path(p) for p in argv[1:]]
    if not targets:
        print("用法：python scripts/check-apk.py <apk...>", file=sys.stderr)
        return 2

    apksigner = find_apksigner()
    failed = 0

    for apk in targets:
        if not apk.exists():
            print(f"{apk}: 文件不存在", file=sys.stderr)
            failed += 1
            continue
        if "unsigned" in apk.name.lower():
            print(f"{apk.name}: 文件名含 unsigned，说明签名配置没生效", file=sys.stderr)
            failed += 1
            continue

        size_mb = apk.stat().st_size / 1024 / 1024
        ok = verify_with_apksigner(apk, apksigner) if apksigner else None
        if ok is None:
            ok, scheme = detect_manually(apk)
            if ok:
                print(f"  {apk.name}: 已签名（{scheme}）")
        if ok:
            print(f"{apk.name}: 可通过，{size_mb:.1f} MB")
        else:
            print(f"{apk.name}: 未签名，无法安装", file=sys.stderr)
            failed += 1

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
