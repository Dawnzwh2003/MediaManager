//! Bounded local ffprobe/FFmpeg operations. No database access and no shell.
mod process;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    ToolMissing,
    ProcessFailed,
    InvalidMetadata,
    UnsupportedCodec,
    OutputInvalid,
    Timeout,
    Cancelled,
    SourceChanged,
    UnsafePath,
    CacheIo,
}
impl Failure {
    pub fn code(self) -> &'static str {
        match self {
            Self::ToolMissing => "ffmpeg_missing",
            Self::ProcessFailed => "ffmpeg_failure",
            Self::InvalidMetadata => "invalid_metadata",
            Self::UnsupportedCodec => "unsupported_codec",
            Self::OutputInvalid => "thumbnail_generation_failure",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::SourceChanged => "source_changed",
            Self::UnsafePath => "unsafe_path",
            Self::CacheIo => "cache_io",
        }
    }
}
#[derive(Debug, Clone)]
pub struct Metadata {
    pub width: i64,
    pub height: i64,
    pub duration: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub frame_rate: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Toolchain {
    pub ffprobe: PathBuf,
    pub ffmpeg: PathBuf,
    pub timeout: Duration,
}
impl Default for Toolchain {
    fn default() -> Self {
        let root = std::env::var_os("MEDIA_MANAGER_FFMPEG_DIR").map(PathBuf::from);
        Self {
            ffprobe: root.as_ref().map_or_else(
                || "ffprobe".into(),
                |p| {
                    p.join(if cfg!(windows) {
                        "ffprobe.exe"
                    } else {
                        "ffprobe"
                    })
                },
            ),
            ffmpeg: root.as_ref().map_or_else(
                || "ffmpeg".into(),
                |p| {
                    p.join(if cfg!(windows) {
                        "ffmpeg.exe"
                    } else {
                        "ffmpeg"
                    })
                },
            ),
            timeout: Duration::from_secs(30),
        }
    }
}
// Exclude playlists, concat, image sequences and network demuxers. Only single-file media.
const FORMATS: &str = "mov,matroska,webm,avi,jpeg_pipe,png_pipe,webp_pipe,gif,bmp_pipe";
fn image_format(source: &Path) -> Result<&'static str, Failure> {
    match source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => Ok("jpeg_pipe"),
        "png" => Ok("png_pipe"),
        "webp" => Ok("webp_pipe"),
        "gif" => Ok("gif"),
        "bmp" => Ok("bmp_pipe"),
        _ => Err(Failure::InvalidMetadata),
    }
}
impl Toolchain {
    pub fn probe(
        &self,
        source: &Path,
        video: bool,
        cancel: &AtomicBool,
    ) -> Result<Metadata, Failure> {
        let mut command = std::process::Command::new(&self.ffprobe);
        command.args(["-max_alloc", "67108864"]);
        if !video {
            command.args(["-f", image_format(source)?]);
        }
        command.args(["-v","error","-threads","1","-protocol_whitelist","file", "-format_whitelist",FORMATS,
            "-probesize","5000000","-analyzeduration","5000000", "-show_entries",
            "stream=codec_type,codec_name,width,height,duration,avg_frame_rate:stream_disposition=attached_pic:format=duration,format_name",
            "-of","json","-i"]).arg(source);
        parse(&process::run(command, self.timeout, cancel)?, video)
    }
    pub fn thumbnail(
        &self,
        source: &Path,
        metadata: &Metadata,
        video: bool,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, Failure> {
        let seek = if video {
            metadata.duration.unwrap_or(0.0) * 0.1
        } else {
            0.0
        };
        let generate = |seek: f64| {
            let mut command = std::process::Command::new(&self.ffmpeg);
            command.args(["-max_alloc", "67108864"]);
            if !video {
                command.args(["-f", image_format(source)?]);
            }
            command.args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-threads",
                "1",
                "-filter_threads",
                "1",
                "-protocol_whitelist",
                "file",
                "-format_whitelist",
                FORMATS,
                "-max_pixels",
                "100000000",
            ]);
            if seek > 0.0 {
                command.args(["-ss", &format!("{seek:.6}")]);
            }
            command.arg("-i").arg(source).args([
                "-map",
                "0:V:0",
                "-frames:v",
                "1",
                "-an",
                "-sn",
                "-dn",
                "-vf",
                "scale=320:320:force_original_aspect_ratio=decrease",
                "-threads",
                "1",
                "-c:v",
                "mjpeg",
                "-q:v",
                "4",
                "-f",
                "image2pipe",
                "pipe:1",
            ]);
            let bytes = process::run(command, self.timeout, cancel)?;
            if !valid_thumbnail(&bytes) {
                return Err(Failure::OutputInvalid);
            }
            Ok(bytes)
        };
        match generate(seek) {
            Err(Failure::OutputInvalid | Failure::ProcessFailed) if seek > 0.0 => generate(0.0),
            result => result,
        }
    }
}
pub(crate) fn valid_thumbnail(bytes: &[u8]) -> bool {
    if bytes.len() > process::MAX_OUTPUT
        || !bytes.starts_with(&[0xff, 0xd8])
        || !bytes.ends_with(&[0xff, 0xd9])
    {
        return false;
    }
    let mut offset = 2;
    let mut dimensions = false;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xff {
            return false;
        }
        let marker = bytes[offset + 1];
        if marker == 0xda {
            return dimensions;
        } // Start of scan, followed by entropy-coded data.
        let length = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
        if length < 2 || offset + 2 + length > bytes.len() {
            return false;
        }
        if matches!(marker, 0xc0..=0xc2) {
            if length < 8 {
                return false;
            }
            let h = u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]);
            let w = u16::from_be_bytes([bytes[offset + 7], bytes[offset + 8]]);
            if !(1..=320).contains(&w) || !(1..=320).contains(&h) {
                return false;
            }
            dimensions = true;
        }
        offset += 2 + length;
    }
    false
}
fn number(v: &Value) -> Option<f64> {
    let n = v.as_f64().or_else(|| v.as_str()?.parse().ok())?;
    (n.is_finite() && n > 0.0).then_some(n)
}
fn name(v: &Value) -> Option<String> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .map(str::to_owned)
}
fn parse(bytes: &[u8], video: bool) -> Result<Metadata, Failure> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Failure::InvalidMetadata)?;
    let streams = value["streams"]
        .as_array()
        .ok_or(Failure::InvalidMetadata)?;
    let stream = streams
        .iter()
        .find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
        .ok_or(Failure::InvalidMetadata)?;
    let width = stream["width"]
        .as_i64()
        .filter(|n| (1..=32768).contains(n))
        .ok_or(Failure::InvalidMetadata)?;
    let height = stream["height"]
        .as_i64()
        .filter(|n| (1..=32768).contains(n))
        .ok_or(Failure::InvalidMetadata)?;
    if width * height > 100_000_000 {
        return Err(Failure::InvalidMetadata);
    }
    let codec = name(&stream["codec_name"])
        .filter(|s| s != "unknown")
        .ok_or(Failure::UnsupportedCodec)?;
    let frame_rate = stream["avg_frame_rate"].as_str().and_then(|s| {
        let (a, b) = s.split_once('/')?;
        let n = a.parse::<f64>().ok()? / b.parse::<f64>().ok()?;
        (n.is_finite() && n > 0.0).then_some(n)
    });
    Ok(Metadata {
        width,
        height,
        duration: video
            .then(|| number(&stream["duration"]).or_else(|| number(&value["format"]["duration"])))
            .flatten(),
        video_codec: video.then_some(codec),
        audio_codec: streams
            .iter()
            .find(|s| s["codec_type"] == "audio")
            .and_then(|s| name(&s["codec_name"])),
        container: name(&value["format"]["format_name"]),
        frame_rate: if video { frame_rate } else { None },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_or_fake_jpeg_is_not_a_ready_thumbnail() {
        assert!(!valid_thumbnail(b""));
        assert!(!valid_thumbnail(&[0xff, 0xd8, 0xff, 0xd9]));
        assert!(!valid_thumbnail(&[
            0xff, 0xd8, 0xff, 0xc0, 0xff, 0xff, 0xff, 0xd9
        ]));
    }
    #[test]
    fn probe_parser_handles_unknown_duration_and_rejects_bad_dimensions_and_codecs() {
        let json = br#"{"streams":[{"codec_type":"video","codec_name":"h264","width":640,"height":360,"avg_frame_rate":"30000/1001","duration":"N/A"}],"format":{"duration":"10.5","format_name":"mov"}}"#;
        let m = parse(json, true).unwrap();
        assert_eq!((m.width, m.height, m.duration), (640, 360, Some(10.5)));
        assert!((m.frame_rate.unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse(json, false).unwrap().duration, None);
        let mut v: Value = serde_json::from_slice(json).unwrap();
        v["format"]["duration"] = "NaN".into();
        assert_eq!(
            parse(&serde_json::to_vec(&v).unwrap(), true)
                .unwrap()
                .duration,
            None
        );
        v["streams"][0]["codec_name"] = "unknown".into();
        assert_eq!(
            parse(&serde_json::to_vec(&v).unwrap(), true).unwrap_err(),
            Failure::UnsupportedCodec
        );
        v["streams"][0]["width"] = 0.into();
        assert_eq!(
            parse(&serde_json::to_vec(&v).unwrap(), true).unwrap_err(),
            Failure::InvalidMetadata
        );
        assert_eq!(
            parse(b"corrupt", true).unwrap_err(),
            Failure::InvalidMetadata
        );
    }
}
