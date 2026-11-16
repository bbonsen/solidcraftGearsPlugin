//! SolidCraft gear plugin: entry `spur` builds an involute spur gear as a native
//! sketch + extrude, so the result is an ordinary editable feature.

pub mod gear;

use gear::SpurGear;
use serde_json::{json, Value};

/// A failure reported back to the host, mirroring the WIT `plugin-error` cases we use.
#[derive(Debug)]
pub enum Failure {
    Input(&'static str, String),
    Command(String, String),
    Cancelled,
}

/// The host calls the plugin needs. Lets `build` run in native tests with a fake host.
pub trait Host {
    fn run(&mut self, command: &str, args: Value) -> Result<Value, Failure>;
    fn progress(&mut self, fraction: f32, message: &str);
    fn cancelled(&mut self) -> bool;
}

fn num(params: &Value, key: &'static str, default: f64) -> Result<f64, Failure> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| Failure::Input(key, "must be a number".into())),
    }
}

fn int(params: &Value, key: &'static str, default: u32) -> Result<u32, Failure> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .and_then(|x| u32::try_from(x).ok())
            .ok_or_else(|| Failure::Input(key, "must be a whole number".into())),
    }
}

/// Build the gear. `params` is the entry's JSON input.
pub fn build(host: &mut dyn Host, params: &Value) -> Result<Value, Failure> {
    let g = SpurGear {
        teeth: int(params, "teeth", 20)?,
        module: num(params, "module", 2.0)?,
        pressure_angle: num(params, "pressure_angle", 20.0)?,
        backlash: num(params, "backlash", 0.0)?,
        flank_points: int(params, "flank_points", 8)?,
    };
    let thickness = num(params, "thickness", 10.0)?;
    if !(thickness > 0.0 && thickness <= 10_000.0) {
        return Err(Failure::Input("thickness", "must be more than 0 and at most 10000 mm".into()));
    }
    let bore = num(params, "bore", 0.0)?;
    let plane = match params.get("plane").and_then(Value::as_str).unwrap_or("XY") {
        p @ ("XY" | "XZ" | "YZ") => p,
        _ => return Err(Failure::Input("plane", "must be XY, XZ or YZ".into())),
    };

    let points = g.profile().map_err(|(f, r)| Failure::Input(f, r))?;
    let c = g.circles();
    if bore < 0.0 || bore >= 2.0 * c.root * 0.9 {
        return Err(Failure::Input("bore", format!("must be 0 (none) or less than {:.3} mm", 2.0 * c.root * 0.9)));
    }

    let name = format!("Spur gear z{} m{}", g.teeth, g.module);
    host.progress(0.1, "sketching profile");
    host.run("sketch.create", json!({"plane": plane, "name": name}))?;
    let pts: Vec<Value> = points.iter().map(|p| json!([round(p[0]), round(p[1])])).collect();
    host.run("sketch.line", json!({"points": pts, "closed": true}))?;
    if bore > 0.0 {
        host.run("sketch.circle.center", json!({"center": [0, 0], "diameter": bore}))?;
    }
    host.run("sketch.finish", json!({}))?;
    if host.cancelled() {
        return Err(Failure::Cancelled);
    }
    host.progress(0.6, "extruding");
    host.run("solid.extrude", json!({"distance": thickness, "operation": "new", "body_name": name}))?;
    host.progress(1.0, "done");

    let undercut = f64::from(g.teeth) < g.min_teeth_without_undercut();
    Ok(json!({
        "body": name,
        "teeth": g.teeth,
        "module_mm": g.module,
        "pressure_angle_deg": g.pressure_angle,
        "pitch_diameter_mm": 2.0 * c.pitch,
        "tip_diameter_mm": 2.0 * c.tip,
        "root_diameter_mm": 2.0 * c.root,
        "base_diameter_mm": 2.0 * c.base,
        "centre_distance_note": "two meshing gears sit (z1 + z2) * module / 2 apart",
        "profile_points": points.len(),
        "warnings": if undercut {
            vec![format!("{} teeth is below {:.1}: a real cut gear would be undercut; this profile is not", g.teeth, g.min_teeth_without_undercut())]
        } else { vec![] },
    }))
}

/// Micrometre precision keeps the JSON small.
fn round(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

#[cfg(target_arch = "wasm32")]
mod component {
    wit_bindgen::generate!({ world: "command-plugin", path: "wit" });

    use super::{Failure, Host};
    use craft::plugin::types::{CommandFailure, FieldFailure, PluginError};
    use craft::plugin::{commands, control, log};
    use serde_json::Value;

    struct Wasm;
    impl Host for Wasm {
        fn run(&mut self, command: &str, args: Value) -> Result<Value, Failure> {
            match commands::run(command, &args.to_string()) {
                Ok(text) => Ok(serde_json::from_str(&text).unwrap_or(Value::Null)),
                Err(e) => Err(Failure::Command(command.to_string(), format!("{e:?}"))),
            }
        }
        fn progress(&mut self, fraction: f32, message: &str) {
            control::progress(fraction, message);
        }
        fn cancelled(&mut self) -> bool {
            control::cancelled()
        }
    }

    struct Plugin;
    impl exports::craft::plugin::entry::Guest for Plugin {
        fn execute(entry: String, params: String) -> Result<String, PluginError> {
            if entry != "spur" {
                return Err(PluginError::InvalidInput(FieldFailure { field: "entry".into(), reason: format!("unknown entry {entry}") }));
            }
            let params: Value = serde_json::from_str(&params).map_err(|e| {
                PluginError::InvalidInput(FieldFailure { field: "params".into(), reason: e.to_string() })
            })?;
            match super::build(&mut Wasm, &params) {
                Ok(v) => Ok(v.to_string()),
                Err(Failure::Input(field, reason)) => {
                    Err(PluginError::InvalidInput(FieldFailure { field: field.into(), reason }))
                }
                Err(Failure::Command(command, reason)) => {
                    log::log(log::Level::Error, &format!("{command} failed: {reason}"));
                    Err(PluginError::CommandFailed(CommandFailure { command, reason }))
                }
                Err(Failure::Cancelled) => Err(PluginError::Cancelled),
            }
        }
    }

    export!(Plugin);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake(Vec<(String, Value)>);
    impl Host for Fake {
        fn run(&mut self, command: &str, args: Value) -> Result<Value, Failure> {
            self.0.push((command.into(), args));
            Ok(json!({}))
        }
        fn progress(&mut self, _: f32, _: &str) {}
        fn cancelled(&mut self) -> bool {
            false
        }
    }

    #[test]
    fn issues_sketch_then_extrude() {
        let mut h = Fake::default();
        let out = build(&mut h, &json!({"teeth": 24, "module": 1.5, "bore": 8, "thickness": 6})).unwrap();
        let cmds: Vec<&str> = h.0.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(cmds, ["sketch.create", "sketch.line", "sketch.circle.center", "sketch.finish", "solid.extrude"]);
        assert_eq!(h.0[1].1["closed"], json!(true));
        assert_eq!(out["pitch_diameter_mm"], json!(36.0));
        assert_eq!(h.0[4].1["distance"], json!(6.0));
    }

    #[test]
    fn bad_input_issues_no_commands() {
        for p in [json!({"teeth": 3}), json!({"plane": "QQ"}), json!({"bore": 1000}), json!({"thickness": -1}), json!({"module": "x"})] {
            let mut h = Fake::default();
            assert!(build(&mut h, &p).is_err(), "{p}");
            assert!(h.0.is_empty());
        }
    }
}
