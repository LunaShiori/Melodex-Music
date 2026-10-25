#!/usr/bin/env python3
"""生成 THIRD_PARTY_LICENSES.txt。

Melodex 直接依赖清单不需要联网扫描：直接依赖是人工可审计的（数量少、变动慢），
所以这里从各清单文件里抽取「直接依赖」，再用内置的许可证表补齐标识符。
传递依赖的许可证由 cargo-deny / 各发行包的自带声明共同覆盖，详见 README。

用法：
    python3 scripts/gen_third_party.py            # 打印到 stdout
    python3 scripts/gen_third_party.py -o FILE    # 写入文件
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# 直接依赖 -> (许可证标识, 主页)。新增依赖时同步补一行。
RUST_LICENSES = {
    "tauri": ("MIT OR Apache-2.0", "https://tauri.app"),
    "tauri-build": ("MIT OR Apache-2.0", "https://tauri.app"),
    "serde": ("MIT OR Apache-2.0", "https://serde.rs"),
    "serde_json": ("MIT OR Apache-2.0", "https://serde.rs"),
    "thiserror": ("MIT OR Apache-2.0", "https://github.com/dtolnay/thiserror"),
    "tokio": ("MIT", "https://tokio.rs"),
    "reqwest": ("MIT OR Apache-2.0", "https://github.com/seanmonstar/reqwest"),
    "async-trait": ("MIT OR Apache-2.0", "https://github.com/dtolnay/async-trait"),
    "futures-util": (
        "MIT OR Apache-2.0",
        "https://github.com/rust-lang/futures-rs",
    ),
    "percent-encoding": (
        "MIT OR Apache-2.0",
        "https://github.com/servo/rust-url",
    ),
}

NODE_LICENSES = {
    "vue": ("MIT", "https://vuejs.org"),
    "pinia": ("MIT", "https://pinia.vuejs.org"),
    "vue-router": ("MIT", "https://router.vuejs.org"),
    "vite": ("MIT", "https://vite.dev"),
    "typescript": ("Apache-2.0", "https://www.typescriptlang.org"),
    "vue-tsc": ("MIT", "https://github.com/vuejs/language-tools"),
    "@vitejs/plugin-vue": ("MIT", "https://github.com/vitejs/vite-plugin-vue"),
    "@tauri-apps/api": ("MIT OR Apache-2.0", "https://tauri.app"),
    "@tauri-apps/cli": ("MIT OR Apache-2.0", "https://tauri.app"),
    "@types/node": ("MIT", "https://github.com/DefinitelyTyped/DefinitelyTyped"),
}

CARGO_MANIFESTS = [
    ("melodex-core", ROOT / "crates/melodex-core/Cargo.toml"),
    ("melodex (Tauri shell)", ROOT / "src-tauri/Cargo.toml"),
]


def cargo_deps(manifest: Path) -> list[tuple[str, str]]:
    """极简 TOML 读取：只抽 [dependencies] / [build-dependencies] 的键与版本。"""
    if not manifest.exists():
        return []
    deps: list[tuple[str, str]] = []
    section = ""
    for raw in manifest.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].rstrip()
        if not line.strip():
            continue
        header = re.match(r"^\[(.+)]$", line.strip())
        if header:
            section = header.group(1)
            continue
        if section not in ("dependencies", "build-dependencies"):
            continue
        m = re.match(r'^([A-Za-z0-9_\-]+)\s*=\s*(.+)$', line.strip())
        if not m:
            continue
        name, spec = m.group(1), m.group(2)
        version = re.search(r'version\s*=\s*"([^"]+)"', spec) or re.search(r'"([^"]+)"', spec)
        deps.append((name, version.group(1) if version else "?"))
    return deps


def node_deps(manifest: Path) -> list[tuple[str, str, str]]:
    if not manifest.exists():
        return []
    data = json.loads(manifest.read_text(encoding="utf-8"))
    out: list[tuple[str, str, str]] = []
    for key, group in (("dependencies", "runtime"), ("devDependencies", "build")):
        for name, version in sorted(data.get(key, {}).items()):
            out.append((name, version, group))
    return out


def render() -> str:
    lines: list[str] = []
    add = lines.append

    add("THIRD-PARTY SOFTWARE NOTICES")
    add("Melodex · 多源音乐聚合搜索与下载")
    add("")
    add("This file lists the direct third-party components Melodex depends on, the")
    add("license each one is distributed under, and where to obtain its full text.")
    add("")
    add("Regenerate with:  python3 scripts/gen_third_party.py -o THIRD_PARTY_LICENSES.txt")
    add("")
    add("Transitive dependencies are resolved by Cargo.lock and package-lock.json.")
    add("To audit the whole tree run, from the repository root:")
    add("")
    add("    cargo install cargo-deny           # MIT OR Apache-2.0")
    add("    cargo deny check licenses")
    add("    npx license-checker --summary      # for the npm tree")
    add("")
    add("=" * 78)
    add("RUST CRATES")
    add("=" * 78)
    add("")

    for label, manifest in CARGO_MANIFESTS:
        deps = cargo_deps(manifest)
        if not deps:
            continue
        add(f"-- {label} ({manifest.relative_to(ROOT).as_posix()})")
        add("")
        add(f"{'component':<24}{'version':<12}{'license':<24}homepage")
        add("-" * 78)
        for name, version in deps:
            license_id, homepage = RUST_LICENSES.get(name, ("see crates.io", ""))
            add(f"{name:<24}{version:<12}{license_id:<24}{homepage}")
        add("")

    add("=" * 78)
    add("NPM PACKAGES")
    add("=" * 78)
    add("")
    add(f"{'component':<26}{'version':<16}{'scope':<10}{'license':<20}homepage")
    add("-" * 78)
    for name, version, group in node_deps(ROOT / "package.json"):
        license_id, homepage = NODE_LICENSES.get(name, ("see npmjs.com", ""))
        add(f"{name:<26}{version:<16}{group:<10}{license_id:<20}{homepage}")
    add("")

    add("=" * 78)
    add("FULL LICENSE TEXTS")
    add("=" * 78)
    add("")
    add("Apache License 2.0   https://www.apache.org/licenses/LICENSE-2.0")
    add("MIT License         https://opensource.org/license/mit")
    add("")
    add("Components marked \"MIT OR Apache-2.0\" are dual-licensed; Melodex consumes")
    add("them under the Apache-2.0 terms, which keeps a single inbound license for")
    add("the whole product.")
    add("")
    add("Melodex itself is licensed under Apache-2.0; see LICENSE and NOTICE.")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="生成第三方许可清单")
    parser.add_argument("-o", "--output", help="写入文件（默认输出到 stdout）")
    args = parser.parse_args()
    text = render()
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
        print(f"已写入 {args.output}（{len(text.splitlines())} 行）", file=sys.stderr)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
