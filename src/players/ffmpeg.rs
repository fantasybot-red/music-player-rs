use anyhow::Result;
use async_trait::async_trait;
use songbird::input::{
    AudioStream, AudioStreamError, AuxMetadata, ChildContainer, Compose, Input, RawAdapter,
};
use std::{
    io::{self, Read},
    process::{Command, Stdio},
    thread,
};
use symphonia::core::io::{MediaSource, ReadOnlySource};

enum FfmpegInput {
    Url,
    Pipe(Option<Box<dyn Read + Send + 'static>>),
}

pub struct Ffmpeg {
    internal: Command,
    input: FfmpegInput,
}

impl Ffmpeg {
    pub fn new(url: impl AsRef<str>) -> Result<Self> {
        let mut internal = Command::new("ffmpeg");
        internal
            .args([
                "-nostdin",
                "-fflags",
                "+nobuffer",
                "-probesize",
                "32",
                "-analyzeduration",
                "0",
                "-threads",
                "1",
                "-loglevel",
                "warning",
                "-reconnect",
                "1",
                "-reconnect_streamed",
                "1",
                "-reconnect_delay_max",
                "3",
                "-reconnect_max_retries",
                "2",
                "-i",
                url.as_ref(),
                "-map",
                "0:a:0",
                "-vn",
                "-f",
                "f32le",
                "-ac",
                "2",
                "-ar",
                "48000",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        Ok(Self {
            internal,
            input: FfmpegInput::Url,
        })
    }

    pub fn new_pipe<R>(reader: R) -> Result<Self>
    where
        R: Read + Send + 'static,
    {
        let mut internal = Command::new("ffmpeg");
        internal
            .args([
                "-nostdin",
                "-threads",
                "1",
                "-loglevel",
                "warning",
                "-i",
                "pipe:0",
                "-map",
                "0:a:0",
                "-vn",
                "-f",
                "f32le",
                "-ac",
                "2",
                "-ar",
                "48000",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        Ok(Self {
            internal,
            input: FfmpegInput::Pipe(Some(Box::new(reader))),
        })
    }
}

#[async_trait]
impl Compose for Ffmpeg {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let reader = match &mut self.input {
            FfmpegInput::Url => None,
            FfmpegInput::Pipe(reader) => Some(reader.take().ok_or_else(|| {
                AudioStreamError::Fail(Box::new(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "ffmpeg pipe input has already been consumed",
                )))
            })?),
        };

        let mut child = self
            .internal
            .spawn()
            .map_err(|e| AudioStreamError::Fail(e.into()))?;

        if let Some(reader) = reader {
            let stdin = child.stdin.take().ok_or_else(|| {
                AudioStreamError::Fail(Box::new(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ffmpeg stdin pipe was not available",
                )))
            })?;

            thread::Builder::new()
                .name("ffmpeg-stdin".to_owned())
                .spawn(move || {
                    let mut reader = reader;
                    let mut stdin = stdin;
                    let _ = io::copy(&mut reader, &mut stdin);
                })
                .map_err(|e| AudioStreamError::Fail(e.into()))?;
        }

        let child_container = ChildContainer::from(child);
        let source = ReadOnlySource::new(child_container);
        let raw_audio = RawAdapter::new(source, 48_000, 2);
        Ok(AudioStream {
            input: Box::new(raw_audio),
        })
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        return Ok(AuxMetadata {
            ..Default::default()
        });
    }
}

impl From<Ffmpeg> for Input {
    fn from(value: Ffmpeg) -> Self {
        Input::Lazy(Box::new(value))
    }
}
