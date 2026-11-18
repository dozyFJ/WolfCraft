use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub type MediaResult<T> = Result<T, String>;

#[derive(Clone)]
pub struct Clip {
    pub path: PathBuf,
    pub duration: f64,
    pub start: f64,
    pub end: f64,
    pub audio: bool,
}

impl Clip {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn length(&self) -> f64 {
        self.end - self.start
    }
}

fn run(command: &mut Command) -> MediaResult<Output> {
    let output = command
        .output()
        .map_err(|error| format!("Не удалось запустить {:?}: {error}", command.get_program()))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn program(name: &str) -> Command {
    #[cfg(target_os = "android")]
    return Command::new(crate::android::executable(name));

    #[cfg(not(target_os = "android"))]
    Command::new(name)
}

#[cfg(target_os = "android")]
const VIDEO_ENCODER: [&str; 4] = ["-c:v", "mpeg4", "-q:v", "4"];
#[cfg(not(target_os = "android"))]
const VIDEO_ENCODER: [&str; 4] = ["-c:v", "libx264", "-preset", "fast"];

pub fn probe(path: PathBuf) -> MediaResult<Clip> {
    let output = run(program("ffprobe")
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(&path))?;
    let info: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let streams = info["streams"]
        .as_array()
        .ok_or("Файл не содержит дорожек")?;
    let video = streams
        .iter()
        .find(|stream| {
            stream["codec_type"] == "video" && stream["disposition"]["attached_pic"] != 1
        })
        .ok_or("Файл не содержит видео")?;
    let duration = [&video["duration"], &info["format"]["duration"]]
        .into_iter()
        .filter_map(serde_json::Value::as_str)
        .filter_map(|value| value.parse::<f64>().ok())
        .find(|value| value.is_finite() && *value >= 0.04)
        .ok_or("Не удалось определить длительность видео")?;
    let audio = streams.iter().any(|stream| stream["codec_type"] == "audio");
    Ok(Clip {
        path,
        duration,
        start: 0.0,
        end: duration,
        audio,
    })
}

pub fn frame(path: &Path, time: f64) -> MediaResult<image::RgbaImage> {
    let output = run(program("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{time:.4}"), "-i"])
        .arg(path)
        .args([
            "-map",
            "0:V:0",
            "-frames:v",
            "1",
            "-vf",
            "scale=800:450:force_original_aspect_ratio=decrease:force_divisible_by=2,setsar=1",
            "-threads",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "pipe:1",
        ]))?;
    image::load_from_memory(&output.stdout)
        .map(|image| image.into_rgba8())
        .map_err(|error| format!("Кадр недоступен: {error}"))
}

pub fn export(clips: &[Clip], destination: &Path, progress: impl Fn(usize)) -> MediaResult<()> {
    let folder = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temporary = tempfile::tempdir_in(folder).map_err(|error| error.to_string())?;
    let mut manifest = String::from("ffconcat version 1.0\n");

    for (index, clip) in clips.iter().enumerate() {
        progress(index + 1);
        let name = format!("part-{index}.mp4");
        let mut command = program("ffmpeg");
        command
            .args([
                "-nostdin",
                "-v",
                "error",
                "-filter_threads",
                "1",
                "-ss",
                &format!("{:.4}", clip.start),
                "-i",
            ])
            .arg(&clip.path);
        if !clip.audio {
            command.args(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"]);
        }
        let audio = if clip.audio { "0:a:0" } else { "1:a:0" };
        run(command.args(["-map", "0:V:0", "-map", audio, "-t", &format!("{:.4}", clip.length()),
            "-vf", "scale=1280:720:force_original_aspect_ratio=decrease:force_divisible_by=2,pad=1280:720:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30",
            "-af", "apad", "-threads", "2", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-ar", "48000", "-ac", "2"])
            .args(VIDEO_ENCODER)
            .arg(temporary.path().join(&name)))?;
        manifest.push_str(&format!("file {name}\n"));
    }

    std::fs::write(temporary.path().join("clips.ffconcat"), manifest)
        .map_err(|error| error.to_string())?;
    let result = temporary.path().join("export.mp4");
    run(program("ffmpeg").current_dir(temporary.path()).args([
        "-nostdin",
        "-v",
        "error",
        "-f",
        "concat",
        "-i",
        "clips.ffconcat",
        "-c",
        "copy",
        "-movflags",
        "+faststart",
        "export.mp4",
    ]))?;
    std::fs::rename(result, destination).map_err(|error| error.to_string())
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires ffmpeg and ffprobe on PATH"]
    fn exports_trimmed_clips_with_and_without_audio() {
        let folder = tempfile::tempdir().unwrap();
        let silent = folder.path().join("silent clip.mp4");
        let sound = folder.path().join("sound clip.mp4");

        run(program("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=320x180:r=25:d=2",
                "-threads",
                "1",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&silent))
        .unwrap();
        run(program("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=240x320:r=24:d=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-threads",
                "1",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&sound))
        .unwrap();

        let mut first = probe(sound).unwrap();
        let mut second = probe(silent).unwrap();
        assert!(first.audio);
        assert!(!second.audio);
        first.start = 0.5;
        first.end = 1.5;
        second.end = 0.5;
        assert_eq!(frame(&first.path, first.start).unwrap().height(), 450);

        let result = folder.path().join("finished cut.mp4");
        export(&[first, second], &result, |_| {}).unwrap();
        let rendered = probe(result.clone()).unwrap();
        assert!(rendered.audio);
        assert!((rendered.duration - 1.5).abs() < 0.1);

        let blue = frame(&result, 0.25).unwrap();
        let red = frame(&result, 1.25).unwrap();
        let blue_pixel = blue.get_pixel(400, 225);
        let red_pixel = red.get_pixel(400, 225);
        assert!(blue_pixel[2] > 200 && blue_pixel[0] < 30);
        assert!(red_pixel[0] > 200 && red_pixel[2] < 30);
    }
}
