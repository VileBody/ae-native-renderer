use crate::{VideoFrame, VideoSink, VideoSinkManifest};
use gst::prelude::*;
use std::path::{Path, PathBuf};

pub struct GstMp4VideoSink {
    output: PathBuf,
    pipeline: gst::Pipeline,
    appsrc: gst_app::AppSrc,
    fps: f64,
    frame_duration_ns: u64,
    frames: u32,
    finished: bool,
}

impl GstMp4VideoSink {
    pub fn open(
        output: impl AsRef<Path>,
        width: u32,
        height: u32,
        fps: f64,
    ) -> anyhow::Result<Self> {
        gst::init().map_err(|err| anyhow::anyhow!("failed to initialize GStreamer: {err}"))?;
        anyhow::ensure!(width > 0 && height > 0, "video sink dimensions must be positive");
        anyhow::ensure!(fps > 0.0, "video sink fps must be positive");

        let output = output.as_ref().to_path_buf();
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let pipeline = gst::Pipeline::new();
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .field("width", width as i32)
            .field("height", height as i32)
            .field("framerate", fps_fraction(fps))
            .build();
        let appsrc = gst::ElementFactory::make("appsrc")
            .property("format", gst::Format::Time)
            .property("is-live", false)
            .property("block", true)
            .build()
            .map_err(|err| anyhow::anyhow!("failed to create appsrc: {err}"))?
            .downcast::<gst_app::AppSrc>()
            .map_err(|_| anyhow::anyhow!("appsrc element had unexpected type"))?;
        appsrc.set_caps(Some(&caps));

        let videoconvert = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|err| anyhow::anyhow!("failed to create videoconvert: {err}"))?;
        let encoder = gst::ElementFactory::make("x264enc")
            .property("bitrate", 8_000_u32)
            .property("key-int-max", fps.ceil().max(1.0) as u32)
            .build()
            .map_err(|err| anyhow::anyhow!("failed to create x264enc: {err}"))?;
        let parser = gst::ElementFactory::make("h264parse")
            .build()
            .map_err(|err| anyhow::anyhow!("failed to create h264parse: {err}"))?;
        let muxer = gst::ElementFactory::make("mp4mux")
            .property("faststart", true)
            .build()
            .map_err(|err| anyhow::anyhow!("failed to create mp4mux: {err}"))?;
        let filesink = gst::ElementFactory::make("filesink")
            .property("location", output.display().to_string())
            .build()
            .map_err(|err| anyhow::anyhow!("failed to create filesink: {err}"))?;

        pipeline.add_many(&[
            appsrc.upcast_ref::<gst::Element>(),
            &videoconvert,
            &encoder,
            &parser,
            &muxer,
            &filesink,
        ])?;
        gst::Element::link_many(&[
            appsrc.upcast_ref::<gst::Element>(),
            &videoconvert,
            &encoder,
            &parser,
            &muxer,
            &filesink,
        ])?;
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|err| anyhow::anyhow!("failed to start GStreamer MP4 sink: {err:?}"))?;

        Ok(Self {
            output,
            pipeline,
            appsrc,
            fps,
            frame_duration_ns: (1_000_000_000.0 / fps).round().max(1.0) as u64,
            frames: 0,
            finished: false,
        })
    }
}

impl VideoSink for GstMp4VideoSink {
    fn write_frame(&mut self, frame: &VideoFrame, time: f64) -> anyhow::Result<()> {
        anyhow::ensure!(!self.finished, "cannot write frame after sink finish");
        let expected = (frame.width * frame.height * 4) as usize;
        anyhow::ensure!(
            frame.rgba.len() == expected,
            "invalid RGBA frame size: got {}, expected {}",
            frame.rgba.len(),
            expected
        );

        let mut buffer = gst::Buffer::with_size(frame.rgba.len())
            .map_err(|err| anyhow::anyhow!("failed to allocate GStreamer buffer: {err}"))?;
        {
            let buffer_mut = buffer
                .get_mut()
                .ok_or_else(|| anyhow::anyhow!("failed to get mutable GStreamer buffer"))?;
            let pts = (time.max(0.0) * 1_000_000_000.0).round() as u64;
            buffer_mut.set_pts(gst::ClockTime::from_nseconds(pts));
            buffer_mut.set_duration(gst::ClockTime::from_nseconds(self.frame_duration_ns));
            let mut map = buffer_mut
                .map_writable()
                .map_err(|err| anyhow::anyhow!("failed to map GStreamer buffer: {err}"))?;
            map.as_mut_slice().copy_from_slice(&frame.rgba);
        }

        self.appsrc
            .push_buffer(buffer)
            .map_err(|err| anyhow::anyhow!("failed to push frame into GStreamer appsrc: {err:?}"))?;
        self.frames += 1;
        Ok(())
    }

    fn finish(&mut self) -> anyhow::Result<VideoSinkManifest> {
        if !self.finished {
            self.appsrc
                .end_of_stream()
                .map_err(|err| anyhow::anyhow!("failed to end GStreamer appsrc stream: {err:?}"))?;
            wait_for_sink_eos(&self.pipeline)?;
            self.pipeline
                .set_state(gst::State::Null)
                .map_err(|err| anyhow::anyhow!("failed to stop GStreamer MP4 sink: {err:?}"))?;
            self.finished = true;
        }

        Ok(VideoSinkManifest {
            backend: "gstreamer-appsrc-mp4".to_string(),
            output: self.output.display().to_string(),
            frames: self.frames,
            duration: self.frames as f64 / self.fps,
        })
    }
}

impl Drop for GstMp4VideoSink {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn wait_for_sink_eos(pipeline: &gst::Pipeline) -> anyhow::Result<()> {
    let bus = pipeline
        .bus()
        .ok_or_else(|| anyhow::anyhow!("GStreamer sink pipeline had no bus"))?;
    loop {
        let Some(message) = bus.timed_pop_filtered(
            gst::ClockTime::from_seconds(60),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        ) else {
            anyhow::bail!("timed out waiting for GStreamer MP4 sink EOS");
        };

        match message.view() {
            gst::MessageView::Eos(_) => return Ok(()),
            gst::MessageView::Error(error) => {
                anyhow::bail!(
                    "GStreamer MP4 sink error from {:?}: {} ({:?})",
                    error.src().map(|src| src.path_string()),
                    error.error(),
                    error.debug()
                );
            }
            _ => {}
        }
    }
}

fn fps_fraction(fps: f64) -> gst::Fraction {
    let rounded = fps.round();
    if (fps - rounded).abs() < 0.000_001 {
        gst::Fraction::new(rounded as i32, 1)
    } else {
        gst::Fraction::new((fps * 1000.0).round() as i32, 1000)
    }
}
