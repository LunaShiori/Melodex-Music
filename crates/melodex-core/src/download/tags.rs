use std::path::Path;

use crate::error::Result;

/// 写入音频文件的元信息。
pub struct TagMeta {
    pub title: String,
    pub artists: String,
    pub album: String,
    pub source: String,
    pub cover: Option<Vec<u8>>,
    pub cover_mime: String,
    pub lyric: Option<String>,
}

const FRAME_TITLE: &[u8; 4] = b"TIT2";
const FRAME_ARTIST: &[u8; 4] = b"TPE1";
const FRAME_ALBUM: &[u8; 4] = b"TALB";
const FRAME_COMMENT: &[u8; 4] = b"TXXX";
const FRAME_LYRIC: &[u8; 4] = b"USLT";
const FRAME_PICTURE: &[u8; 4] = b"APIC";

/// 生成 ID3v2.3 标签字节流（含头部）。
///
/// 之所以自己拼而不用第三方库：只需要写 5 个帧，手写可省掉一个编译期依赖，
/// 也让 Android 端的构建链路更简单。
pub fn build_id3_tag(meta: &TagMeta) -> Vec<u8> {
    let mut frames: Vec<u8> = Vec::new();

    if !meta.title.is_empty() {
        frames.extend_from_slice(&text_frame(FRAME_TITLE, &meta.title));
    }
    if !meta.artists.is_empty() {
        frames.extend_from_slice(&text_frame(FRAME_ARTIST, &meta.artists));
    }
    if !meta.album.is_empty() {
        frames.extend_from_slice(&text_frame(FRAME_ALBUM, &meta.album));
    }
    if !meta.source.is_empty() {
        frames.extend_from_slice(&txxx_frame("SOURCE", &meta.source));
    }
    if let Some(ref lyric) = meta.lyric {
        if !lyric.trim().is_empty() {
            frames.extend_from_slice(&uslt_frame(lyric));
        }
    }
    if let Some(ref cover) = meta.cover {
        if !cover.is_empty() {
            let mime = if meta.cover_mime.is_empty() {
                "image/jpeg"
            } else {
                meta.cover_mime.as_str()
            };
            frames.extend_from_slice(&apic_frame(mime, cover));
        }
    }

    let mut tag = Vec::with_capacity(frames.len() + 10);
    tag.extend_from_slice(b"ID3");
    tag.push(3); // v2.3
    tag.push(0); // revision
    tag.push(0); // flags
    tag.extend_from_slice(&syncsafe(frames.len() as u32));
    tag.extend_from_slice(&frames);
    tag
}

fn frame_header(id: &[u8; 4], size: usize) -> Vec<u8> {
    let mut header = Vec::with_capacity(10);
    header.extend_from_slice(id);
    header.extend_from_slice(&(size as u32).to_be_bytes());
    header.extend_from_slice(&[0, 0]);
    header
}

/// 文本帧统一用 UTF-16LE + BOM，避免中文在部分播放器里乱码。
fn text_payload(value: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(value.len() * 2 + 3);
    data.push(0x01);
    data.extend_from_slice(&[0xFF, 0xFE]);
    for unit in value.encode_utf16() {
        data.extend_from_slice(&unit.to_le_bytes());
    }
    data
}

fn text_frame(id: &[u8; 4], value: &str) -> Vec<u8> {
    let payload = text_payload(value);
    let mut out = frame_header(id, payload.len());
    out.extend_from_slice(&payload);
    out
}

fn txxx_frame(description: &str, value: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(0x01);
    payload.extend_from_slice(&[0xFF, 0xFE]);
    for unit in description.encode_utf16() {
        payload.extend_from_slice(&unit.to_le_bytes());
    }
    payload.extend_from_slice(&[0x00, 0x00]);
    for unit in value.encode_utf16() {
        payload.extend_from_slice(&unit.to_le_bytes());
    }
    let mut out = frame_header(FRAME_COMMENT, payload.len());
    out.extend_from_slice(&payload);
    out
}

fn uslt_frame(lyric: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(0x01);
    payload.extend_from_slice(b"chi");
    payload.extend_from_slice(&[0xFF, 0xFE]);
    payload.extend_from_slice(&[0x00, 0x00]); // 空描述
    for unit in lyric.encode_utf16() {
        payload.extend_from_slice(&unit.to_le_bytes());
    }
    let mut out = frame_header(FRAME_LYRIC, payload.len());
    out.extend_from_slice(&payload);
    out
}

fn apic_frame(mime: &str, image: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(0x00); // latin1 for mime/description
    payload.extend_from_slice(mime.as_bytes());
    payload.push(0x00);
    payload.push(0x03); // 封面（正面）
    payload.push(0x00); // 空描述
    payload.extend_from_slice(image);
    let mut out = frame_header(FRAME_PICTURE, payload.len());
    out.extend_from_slice(&payload);
    out
}

fn syncsafe(value: u32) -> [u8; 4] {
    [
        ((value >> 21) & 0x7F) as u8,
        ((value >> 14) & 0x7F) as u8,
        ((value >> 7) & 0x7F) as u8,
        (value & 0x7F) as u8,
    ]
}

fn read_syncsafe(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32 & 0x7F) << 21)
        | ((bytes[1] as u32 & 0x7F) << 14)
        | ((bytes[2] as u32 & 0x7F) << 7)
        | (bytes[3] as u32 & 0x7F)
}

/// 跳过已有的 ID3v2 标签，避免重复写入导致文件越滚越大。
pub fn strip_id3v2(data: &[u8]) -> &[u8] {
    if data.len() < 10 || &data[..3] != b"ID3" {
        return data;
    }
    let size = read_syncsafe(&data[6..10]) as usize;
    let mut total = 10 + size;
    // 头部 flag 0x10 表示带 footer
    if data[5] & 0x10 != 0 {
        total += 10;
    }
    if total >= data.len() {
        return &[];
    }
    &data[total..]
}

/// 为 mp3 文件写入标签：保留原音频数据，仅替换头部标签。
pub fn write_mp3_tags(path: &Path, meta: &TagMeta) -> Result<()> {
    let audio = std::fs::read(path)?;
    let body = strip_id3v2(&audio);
    let tag = build_id3_tag(meta);
    let tmp = path.with_extension("melodex-tag");
    let mut out = Vec::with_capacity(tag.len() + body.len());
    out.extend_from_slice(&tag);
    out.extend_from_slice(body);
    std::fs::write(&tmp, &out)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> TagMeta {
        TagMeta {
            title: "晴天".into(),
            artists: "周杰伦".into(),
            album: "叶惠美".into(),
            source: "netease".into(),
            cover: Some(vec![0xFF, 0xD8, 0xFF, 0xE0]),
            cover_mime: "image/jpeg".into(),
            lyric: Some("[00:01.00]故事的小黄花".into()),
        }
    }

    #[test]
    fn builds_valid_header() {
        let tag = build_id3_tag(&meta());
        assert_eq!(&tag[..3], b"ID3");
        assert_eq!(tag[3], 3);
        let size = read_syncsafe(&tag[6..10]) as usize;
        assert_eq!(size + 10, tag.len());
    }

    #[test]
    fn contains_all_frames() {
        let tag = build_id3_tag(&meta());
        for id in [b"TIT2", b"TPE1", b"TALB", b"TXXX", b"USLT", b"APIC"] {
            assert!(
                tag.windows(4).any(|w| w == id),
                "缺少帧 {}",
                String::from_utf8_lossy(id)
            );
        }
    }

    #[test]
    fn strip_removes_existing_tag() {
        let tag = build_id3_tag(&meta());
        let mut file = tag.clone();
        file.extend_from_slice(b"\xFF\xFB\x90\x00AUDIO");
        let body = strip_id3v2(&file);
        assert_eq!(body, b"\xFF\xFB\x90\x00AUDIO");
    }

    #[test]
    fn strip_keeps_plain_mp3() {
        let raw = b"\xFF\xFB\x90\x00AUDIO";
        assert_eq!(strip_id3v2(raw), raw);
    }

    #[test]
    fn writes_tags_to_file() {
        let dir = std::env::temp_dir().join("melodex-tag-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.mp3");
        std::fs::write(&path, b"\xFF\xFB\x90\x00AUDIO").unwrap();
        write_mp3_tags(&path, &meta()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..3], b"ID3");
        assert!(bytes.windows(6).any(|w| w == b"\xFF\xFB\x90\x00AU"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
