//! Extension point for print-failure detection on camera frames.
//!
//! Nothing here detects anything yet. The camera worker already calls
//! [`FrameAnalyzer::analyze`] on every decoded frame when the analyzer reports
//! itself enabled, so a real implementation (e.g. a local ONNX model looking
//! for spaghetti) only has to implement this trait and be passed to
//! `CameraStream::start` instead of [`Disabled`].

#![allow(dead_code)] // fields are read once a real analyzer exists

#[derive(Clone, Debug)]
pub struct Detection {
    pub label: String,
    /// 0.0–1.0
    pub confidence: f32,
}

pub trait FrameAnalyzer: Send {
    fn name(&self) -> &'static str;

    /// When false the camera worker skips `analyze` entirely.
    fn enabled(&self) -> bool;

    /// Inspect one frame, at the resolution chosen in the camera settings.
    /// Runs on the camera thread, so it must keep up with the stream or
    /// sample frames itself.
    fn analyze(&mut self, frame: &image::RgbImage) -> Option<Detection>;
}

/// The analyzer used until detection is implemented.
pub struct Disabled;

impl FrameAnalyzer for Disabled {
    fn name(&self) -> &'static str {
        "None"
    }

    fn enabled(&self) -> bool {
        false
    }

    fn analyze(&mut self, _frame: &image::RgbImage) -> Option<Detection> {
        None
    }
}
