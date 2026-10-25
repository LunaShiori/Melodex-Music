#!/usr/bin/env python3
"""生成/重建 Android CI 签名密钥（PKCS#12）。

⚠️ 只在需要更换 CI 签名密钥时运行一次。密钥一旦变更，已安装的旧版无法覆盖升级，
   必须卸载重装。生成结果会写入 src-tauri/ci-signing/melodex-ci.p12 并提交到仓库。

设计说明：
  Melodex 通过 GitHub Releases 侧载 APK，Android 要求「同一应用必须同一签名」才能覆盖安装，
  所以 CI 不能每次现生成随机密钥。这里改为提交一把公开的 CI 密钥（口令同样公开），
  任何人都能看到——它只用于让 Release 里的 APK 可安装、可升级，不代表任何安全声明。
  若你要上架应用商店，请换成自己的私钥，并通过环境变量覆盖（见 README「Android 发布」）。

用法：
    python scripts/gen-ci-keystore.py            # 覆盖密码 = melodex-ci
    python scripts/gen-ci-keystore.py --password xxx
"""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.serialization import pkcs12
from cryptography.x509.oid import NameOID

DEFAULT_PASSWORD = "melodex-ci"
DEFAULT_ALIAS = "melodex"
DEFAULT_OUT = pathlib.Path("src-tauri/ci-signing/melodex-ci.p12")


def main() -> int:
    parser = argparse.ArgumentParser(description="生成 Android CI 签名密钥")
    parser.add_argument("--password", default=DEFAULT_PASSWORD)
    parser.add_argument("--alias", default=DEFAULT_ALIAS)
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    args = parser.parse_args()

    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)

    subject = issuer = x509.Name(
        [
            x509.NameAttribute(NameOID.COUNTRY_NAME, "CN"),
            x509.NameAttribute(NameOID.STATE_OR_PROVINCE_NAME, "Guangdong"),
            x509.NameAttribute(NameOID.LOCALITY_NAME, "Shenzhen"),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "Melodex"),
            x509.NameAttribute(NameOID.ORGANIZATIONAL_UNIT_NAME, "CI Signing"),
            x509.NameAttribute(NameOID.COMMON_NAME, "Melodex CI"),
        ]
    )
    now = dt.datetime.now(dt.timezone.utc)
    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(issuer)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(now - dt.timedelta(days=1))
        .not_valid_after(now + dt.timedelta(days=10000))
        .add_extension(x509.BasicConstraints(ca=False, path_length=None), critical=True)
        .sign(key, hashes.SHA256())
    )

    blob = pkcs12.serialize_key_and_certificates(
        name=args.alias.encode(),
        key=key,
        cert=cert,
        cas=None,
        encryption_algorithm=serialization.BestAvailableEncryption(args.password.encode()),
    )

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(blob)

    # 回读校验：确认真能解开、别名正确
    _, loaded_cert, _ = pkcs12.load_key_and_certificates(blob, args.password.encode())
    assert loaded_cert is not None, "生成的密钥无法回读"

    print(f"已生成 {out}（{len(blob)} 字节）")
    print(f"  alias    = {args.alias}")
    print(f"  password = {args.password}")
    print(f"  有效期至 = {cert.not_valid_after_utc.date()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
