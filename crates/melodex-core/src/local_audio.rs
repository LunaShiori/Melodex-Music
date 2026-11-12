//! 本地音频文件。
//!
//! 只做两件小事：判断一个路径是不是能播的音频、把「打开方式」传进来的路径洗干净。
//! 不读标签也不解析时长——那需要额外的解码依赖，而这里的目的是让双击歌曲文件能直接播，
//! 标题先用文件名，时长等真正解码时由播放器填。

use std::path::Path;

/// 能交给 `<audio>` / 系统播放器的扩展名。ogg 与 opus 在各平台浏览器里的支持不完全一致，
/// 但列出来不会有害：真播不了时播放器会走自己的错误提示。
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "aac", "wav", "ogg", "opus", "wma"];

/// 这个路径看起来是不是音频文件（只看扩展名，大小写不敏感）。
pub fn is_audio_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// 从命令行参数里挑出音频文件。
///
/// 双击文件打开时，系统会把路径作为参数传进来；第一个参数是可执行文件本身，
/// 其余可能带着 `--flag` 这类开关，所以既要跳过第一个，也要跳过以 `-` 开头的。
pub fn audio_files_from_args<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .skip(1)
        .map(|arg| arg.as_ref().to_string())
        .filter(|arg| !arg.starts_with('-'))
        .filter(|arg| is_audio_file(arg))
        .collect()
}

/// 播放列表里显示的标题：文件名的最后一段，去掉扩展名。
pub fn title_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.trim().to_string())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "未知曲目".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_common_audio_extensions() {
        assert!(is_audio_file("D:/Music/晴天.mp3"));
        assert!(is_audio_file("/home/me/a.FLAC"));
        assert!(is_audio_file("x.m4a"));
        assert!(!is_audio_file("D:/Music/cover.jpg"));
        assert!(!is_audio_file("noext"));
    }

    #[test]
    fn keeps_only_audio_arguments() {
        let args = [
            "melodex.exe",
            "C:/Music/晴天.mp3",
            "--minimized",
            "C:/Music/稻香.flac",
            "readme.txt",
        ];
        assert_eq!(
            audio_files_from_args(args),
            vec![
                "C:/Music/晴天.mp3".to_string(),
                "C:/Music/稻香.flac".to_string()
            ]
        );
    }

    #[test]
    fn title_falls_back_to_placeholder() {
        assert_eq!(title_from_path("/a/b/晴天.mp3"), "晴天");
        assert_eq!(title_from_path(""), "未知曲目");
    }
}
