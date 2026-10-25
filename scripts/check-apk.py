#!/usr/bin/env python3
"""校验产出的 APK 是否真的被签名（未签名的包装不到手机上）。

用法：
    python scripts/check-apk.py out/*.apk

退出码非零表示有文件缺失签名，CI 会因此失败——这是有意的，
避免把装不上的包挂到 Release 上。
"""

from __future__ import annotations

import pathlib
import sys
import zipfile

SIGNATURE_SUFFIXES = (".RSA", ".DSA", ".EC")


def is_signed(apk: pathlib.Path) -> bool:
    try:
        with zipfile.ZipFile(apk) as zf:
            names = zf.namelist()
    except zipfile.BadZipFile:
        print(f"  {apk.name}: 不是合法的 zip/apk", file=sys.stderr)
        return False

    has_block = any(n.startswith("META-INF/") and n.upper().endswith(SIGNATURE_SUFFIXES) for n in names)
    has_sf = any(n.startswith("META-INF/") and n.upper().endswith(".SF") for n in names)
    return has_block and has_sf


def main(argv: list[str]) -> int:
    targets = [pathlib.Path(p) for p in argv[1:]]
    if not targets:
        print("用法：python scripts/check-apk.py <apk...>", file=sys.stderr)
        return 2

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
        if is_signed(apk):
            print(f"{apk.name}: 已签名，{size_mb:.1f} MB")
        else:
            print(f"{apk.name}: 缺少 META-INF 签名块", file=sys.stderr)
            failed += 1

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
