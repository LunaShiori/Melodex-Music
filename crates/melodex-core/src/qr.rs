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
}
