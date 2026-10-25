//! 二维码渲染。
//!
//! 放在核心侧渲染成 SVG：桌面端与 Android 共用同一份实现，前端不需要额外依赖，
//! 也不用担心 canvas 尺寸与设备像素比。

/// 把文本渲染成 SVG 二维码。内容过长（超过二维码容量）时返回 `None`。
pub fn render_svg(content: &str) -> Option<String> {
    use qrcode::render::svg;

    let code = qrcode::QrCode::new(content.as_bytes()).ok()?;
    let raw = code
        .render::<svg::Color>()
        .min_dimensions(232, 232)
        .dark_color(svg::Color("#12121a"))
        .light_color(svg::Color("#ffffff"))
        .build();
    // 去掉 `<?xml ...?>` 声明：内联进 HTML 时它会被当成注释，直接用裸 <svg> 更干净
    let start = raw.find("<svg")?;
    Some(raw[start..].to_string())
}

/// 把字节编成 base64，用于把音源下发的二维码位图内联成 `data:` URI。
///
/// 只为了这一个用途，不值得引入额外依赖，因此自己实现（标准字母表 + `=` 填充）。
pub fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) as usize & 0x3f] as char);
        out.push(TABLE[(triple >> 12) as usize & 0x3f] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(triple >> 6) as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[triple as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_svg() {
        let svg = render_svg("https://music.163.com/login?codekey=abc").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        // 二维码必须有明暗两种颜色，否则扫不出来
        assert!(svg.contains("#12121a"));
    }

    #[test]
    fn rejects_oversized_payload() {
        // 超过二维码最大容量（约 2953 字节）时不应 panic
        let huge = "音".repeat(4000);
        assert!(render_svg(&huge).is_none());
    }

    /// base64 的填充边界最容易错，这里覆盖 1/2/3 字节三种余数。
    #[test]
    fn base64_matches_reference_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    /// PNG 魔数编出来必须以 `iVBORw0KGgo` 开头，否则前端 `<img>` 无法识别
    #[test]
    fn base64_encodes_png_header() {
        let png = [0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        assert!(base64_encode(&png).starts_with("iVBORw0KGgo"));
    }
}
