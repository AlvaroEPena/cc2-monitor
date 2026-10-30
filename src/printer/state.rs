//! Printer state as last reported over MQTT.
//!
//! The printer sends one full status (method 1002) and then small deltas on
//! `api_status`, so the raw JSON is kept and deep-merged rather than mapped to
//! rigid structs. Firmware revisions add and rename fields; a `Value` tree
//! tolerates that, and the accessors below give the UI typed reads.

use std::collections::VecDeque;
use std::time::Instant;

use serde_json::Value;

/// Seconds of temperature history kept for the chart (1 sample/second).
pub const TEMP_HISTORY_SECS: usize = 60 * 60;

#[derive(Default)]
pub struct PrinterState {
    pub attributes: Value,
    pub status: Value,
    pub canvas: Value,
    pub history: Vec<Value>,
    pub files: Vec<Value>,
    pub files_media: String,
    /// Total layers from method 1046, used when status omits `total_layer`.
    pub file_total_layers: Option<i64>,
    pub last_update: Option<Instant>,
    pub temps: TempHistory,
    last_report_id: Option<i64>,
}

impl PrinterState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Replace the whole status from a method 1002 response.
    pub fn set_full_status(&mut self, mut result: Value) {
        normalize(&mut result);
        if let Some(canvas) = result.get("canvas_info").cloned() {
            self.canvas = canvas;
        }
        self.status = result;
        self.last_update = Some(Instant::now());
    }

    /// Merge an `api_status` delta. Returns true when an `auto_report_id` gap
    /// means deltas were missed and a full refresh should be requested.
    pub fn apply_delta(&mut self, msg: &Value) -> bool {
        let mut gap = false;
        if let Some(id) = msg.get("auto_report_id").and_then(Value::as_i64) {
            if let Some(last) = self.last_report_id {
                gap = id != last + 1;
            }
            self.last_report_id = Some(id);
        }
        let Some(result) = msg.get("result") else {
            return gap;
        };
        let mut result = result.clone();
        normalize(&mut result);
        if let Some(canvas) = result.get("canvas_info") {
            deep_merge(&mut self.canvas, canvas.clone());
        }
        deep_merge(&mut self.status, result);
        self.last_update = Some(Instant::now());
        gap
    }

    pub fn has_status(&self) -> bool {
        self.status.is_object()
    }

    // ---- typed accessors ------------------------------------------------

    pub fn f(&self, path: &str) -> Option<f64> {
        get(&self.status, path).and_then(Value::as_f64)
    }

    pub fn i(&self, path: &str) -> Option<i64> {
        get(&self.status, path).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
    }

    pub fn s(&self, path: &str) -> Option<&str> {
        get(&self.status, path).and_then(Value::as_str)
    }

    pub fn attr(&self, path: &str) -> Option<&str> {
        get(&self.attributes, path).and_then(Value::as_str)
    }

    pub fn machine_status(&self) -> Option<i64> {
        self.i("machine_status.status")
    }

    pub fn sub_status(&self) -> i64 {
        self.i("machine_status.sub_status").unwrap_or(0)
    }

    pub fn is_printing(&self) -> bool {
        self.machine_status() == Some(2)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.sub_status(), 2501 | 2502 | 2505)
            || self.s("print_status.state") == Some("paused")
    }

    pub fn progress(&self) -> f64 {
        self.f("print_status.progress")
            .or_else(|| self.f("machine_status.progress"))
            .unwrap_or(0.0)
    }

    pub fn total_layers(&self) -> Option<i64> {
        self.i("print_status.total_layer")
            .filter(|&n| n > 0)
            .or(self.file_total_layers)
    }

    pub fn exceptions(&self) -> Vec<i64> {
        get(&self.status, "machine_status.exception_status")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default()
    }

    /// Fan speed as a 0–100 percentage (the printer reports PWM 0–255).
    pub fn fan_pct(&self, fan: &str) -> Option<f32> {
        self.f(&format!("fans.{fan}.speed"))
            .map(|pwm| (pwm / 255.0 * 100.0).round() as f32)
    }

    pub fn fan_rpm(&self, fan: &str) -> Option<i64> {
        self.i(&format!("fans.{fan}.rpm"))
    }

    pub fn canvas_units(&self) -> Vec<&Value> {
        self.canvas
            .get("canvas_list")
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    }

    /// Record one temperature sample for the chart.
    pub fn sample_temps(&mut self) {
        if !self.has_status() {
            return;
        }
        let sample = TempSample {
            at: Instant::now(),
            nozzle: self.f("extruder.temperature"),
            nozzle_target: self.f("extruder.target"),
            bed: self.f("heater_bed.temperature"),
            bed_target: self.f("heater_bed.target"),
            chamber: self.f("ztemperature_sensor.temperature"),
        };
        self.temps.push(sample);
    }
}

#[derive(Clone, Copy)]
pub struct TempSample {
    pub at: Instant,
    pub nozzle: Option<f64>,
    pub nozzle_target: Option<f64>,
    pub bed: Option<f64>,
    pub bed_target: Option<f64>,
    pub chamber: Option<f64>,
}

#[derive(Default)]
pub struct TempHistory {
    pub samples: VecDeque<TempSample>,
}

impl TempHistory {
    fn push(&mut self, s: TempSample) {
        if self.samples.len() >= TEMP_HISTORY_SECS {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
    }
}

/// Look up a dotted path like `extruder.temperature`.
pub fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |cur, key| cur.get(key))
}

/// Objects merge recursively; anything else (including arrays) replaces.
pub fn deep_merge(base: &mut Value, update: Value) {
    match (base, update) {
        (Value::Object(b), Value::Object(u)) => {
            for (k, v) in u {
                match b.get_mut(&k) {
                    Some(existing) if existing.is_object() && v.is_object() => {
                        deep_merge(existing, v)
                    }
                    _ => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, u) => *b = u,
    }
}

/// Older firmware calls `gcode_move` `gcode_move_inf`.
fn normalize(result: &mut Value) {
    if let Some(obj) = result.as_object_mut() {
        if !obj.contains_key("gcode_move") {
            if let Some(v) = obj.remove("gcode_move_inf") {
                obj.insert("gcode_move".into(), v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn delta_merges_nested_objects() {
        let mut st = PrinterState::default();
        st.set_full_status(json!({
            "extruder": {"temperature": 25.0, "target": 0},
            "gcode_move_inf": {"x": 1.0, "y": 2.0, "z": 3.0},
        }));
        assert_eq!(st.f("gcode_move.x"), Some(1.0));

        st.apply_delta(&json!({"result": {"extruder": {"target": 220}}}));
        assert_eq!(st.f("extruder.temperature"), Some(25.0));
        assert_eq!(st.i("extruder.target"), Some(220));
    }

    #[test]
    fn report_id_gap_is_detected() {
        let mut st = PrinterState::default();
        assert!(!st.apply_delta(&json!({"auto_report_id": 5, "result": {}})));
        assert!(!st.apply_delta(&json!({"auto_report_id": 6, "result": {}})));
        assert!(st.apply_delta(&json!({"auto_report_id": 9, "result": {}})));
    }

    #[test]
    fn fan_pwm_converts_to_percent() {
        let mut st = PrinterState::default();
        st.set_full_status(json!({"fans": {"fan": {"speed": 255}, "aux_fan": {"speed": 128}}}));
        assert_eq!(st.fan_pct("fan"), Some(100.0));
        assert_eq!(st.fan_pct("aux_fan"), Some(50.0));
    }
}
