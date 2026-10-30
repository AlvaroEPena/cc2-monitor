//! Settings persisted between runs (eframe stores them in the OS app-data dir:
//! `%APPDATA%\CC2 Monitor` on Windows, `~/.local/share/cc2monitor` on Linux).

use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "settings";
pub const DEFAULT_ACCESS_CODE: &str = "123456";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub ip: String,
    pub access_code: String,
    pub serial: String,
    pub auto_connect: bool,
    pub camera_on: bool,
    /// Camera frame rate cap; 0 = as fast as the camera sends (~20 fps).
    pub camera_fps: u32,
    /// Camera resolution divisor: 1 full, 2 half, 4 quarter.
    pub camera_scale: u8,
    /// Last plate side used to start a print: false textured (A), true smooth (B).
    pub smooth_plate: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ip: String::new(),
            access_code: DEFAULT_ACCESS_CODE.into(),
            serial: String::new(),
            auto_connect: true,
            camera_on: true,
            camera_fps: 10,
            camera_scale: 1,
            smooth_plate: false,
        }
    }
}
