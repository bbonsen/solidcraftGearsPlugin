//! SolidCraft gear plugin. Entry `spur` builds an involute spur gear as a native sketch +
//! extrude; entry `helical` sweeps the transverse profile along the gear axis with a twist.
//! Either way the result is an ordinary editable feature.

pub mod gear;

use gear::SpurGear;
use serde_json::{json, Value};
use std::f64::consts::PI;

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

/// The plugin's entries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Spur,
    Helical,
}

impl Kind {
    pub fn from_entry(entry: &str) -> Option<Kind> {
        match entry {
            "spur" => Some(Kind::Spur),
            "helical" => Some(Kind::Helical),
            _ => None,
        }
    }
}

/// A straight path from the origin along `plane`'s normal, `length` long, drawn in a sketch on
/// a perpendicular plane: (path sketch plane, start, end). Sketch axes per plane: XY → (+X, +Y),
/// XZ → (+X, -Z), YZ → (+Y, +Z).
fn axis_path(plane: &str, length: f64) -> (&'static str, [f64; 2]) {
    match plane {
        "XZ" => ("XY", [0.0, length]), // normal +Y
        "YZ" => ("XY", [length, 0.0]), // normal +X
        _ => ("XZ", [0.0, -length]),   // XY: normal +Z, which is -y in an XZ sketch
    }
}

/// Build a gear. `params` is the entry's JSON input.
pub fn build(host: &mut dyn Host, kind: Kind, params: &Value) -> Result<Value, Failure> {
    let (helix_angle, right_hand) = match kind {
        Kind::Spur => (0.0, true),
        Kind::Helical => {
            let beta = num(params, "helix_angle", 20.0)?;
            if !(beta.is_finite() && beta > 0.0 && beta <= 45.0) {
                return Err(Failure::Input("helix_angle", "must be more than 0 and at most 45 degrees".into()));
            }
            let right = match params.get("hand").and_then(Value::as_str).unwrap_or("right") {
                "right" => true,
                "left" => false,
                _ => return Err(Failure::Input("hand", "must be right or left".into())),
            };
            (beta, right)
        }
    };
    let g = SpurGear {
        teeth: int(params, "teeth", 20)?,
        module: num(params, "module", 2.0)?,
        pressure_angle: num(params, "pressure_angle", 20.0)?,
        helix_angle,
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
    // Right hand: the teeth turn right-handed about the axis as they advance along it, which is
    // the sweep's positive twist (right hand about the direction of travel).
    let twist_deg = g.twist(thickness).to_degrees() * if right_hand { 1.0 } else { -1.0 };

    let name = match kind {
        Kind::Spur => format!("Spur gear z{} m{}", g.teeth, g.module),
        Kind::Helical => format!(
            "Helical gear z{} m{} {}{}",
            g.teeth,
            g.module,
            if right_hand { "RH" } else { "LH" },
            g.helix_angle
        ),
    };
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
    match kind {
        Kind::Spur => {
            host.progress(0.6, "extruding");
            host.run("solid.extrude", json!({"distance": thickness, "operation": "new", "body_name": name}))?;
        }
        Kind::Helical => {
            let path_name = format!("{name} axis");
            let (path_plane, end) = axis_path(plane, thickness);
            host.run("sketch.create", json!({"plane": path_plane, "name": path_name}))?;
            host.run("sketch.line", json!({"points": [[0, 0], [round(end[0]), round(end[1])]], "ids": ["axis"]}))?;
            host.run("sketch.finish", json!({}))?;
            if host.cancelled() {
                return Err(Failure::Cancelled);
            }
            host.progress(0.5, "sweeping teeth");
            host.run(
                "solid.sweep",
                json!({
                    "sketch": name,
                    "path_sketch": path_name,
                    "path": ["axis"],
                    "twist": format!("{:.6} deg", twist_deg),
                    "operation": "new",
                    "body_name": name,
                }),
            )?;
        }
    }
    host.progress(1.0, "done");

    let undercut = f64::from(g.teeth) < g.min_teeth_without_undercut();
    let mut out = json!({
        "body": name,
        "teeth": g.teeth,
        "module_mm": g.module,
        "pressure_angle_deg": g.pressure_angle,
        "pitch_diameter_mm": 2.0 * c.pitch,
        "tip_diameter_mm": 2.0 * c.tip,
        "root_diameter_mm": 2.0 * c.root,
        "base_diameter_mm": 2.0 * c.base,
        "centre_distance_note": "two meshing gears sit (z1 + z2) * transverse module / 2 apart",
        "profile_points": points.len(),
        "warnings": if undercut {
            vec![format!("{} teeth is below {:.1}: a real cut gear would be undercut; this profile is not", g.teeth, g.min_teeth_without_undercut())]
        } else { vec![] },
    });
    if kind == Kind::Helical {
        out["helix_angle_deg"] = json!(g.helix_angle);
        out["hand"] = json!(if right_hand { "right" } else { "left" });
        out["transverse_module_mm"] = json!(g.transverse_module());
        out["transverse_pressure_angle_deg"] = json!(g.transverse_pressure_angle().to_degrees());
        out["twist_deg"] = json!(twist_deg);
        out["lead_mm"] = json!(PI * 2.0 * c.pitch / g.helix_angle.to_radians().tan());
        out["mating_note"] = json!("a mating gear on a parallel shaft needs the same helix angle and the opposite hand");
    }
    Ok(out)
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
            let Some(kind) = super::Kind::from_entry(&entry) else {
                return Err(PluginError::InvalidInput(FieldFailure { field: "entry".into(), reason: format!("unknown entry {entry}") }));
            };
            let params: Value = serde_json::from_str(&params).map_err(|e| {
                PluginError::InvalidInput(FieldFailure { field: "params".into(), reason: e.to_string() })
            })?;
            match super::build(&mut Wasm, kind, &params) {
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
        let out = build(&mut h, Kind::Spur, &json!({"teeth": 24, "module": 1.5, "bore": 8, "thickness": 6})).unwrap();
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
            assert!(build(&mut h, Kind::Spur, &p).is_err(), "{p}");
            assert!(h.0.is_empty());
        }
    }

    #[test]
    fn helical_sweeps_along_the_axis_with_twist() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Helical, &json!({"teeth": 20, "module": 2, "helix_angle": 30, "thickness": 12})).unwrap();
        let cmds: Vec<&str> = h.0.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(cmds, ["sketch.create", "sketch.line", "sketch.finish", "sketch.create", "sketch.line", "sketch.finish", "solid.sweep"]);
        // XY profile: path drawn on XZ, where world +Z is sketch -y.
        assert_eq!(h.0[3].1["plane"], json!("XZ"));
        assert_eq!(h.0[4].1["points"], json!([[0, 0], [0.0, -12.0]]));
        let sweep = &h.0[6].1;
        assert_eq!(sweep["sketch"], h.0[0].1["name"]);
        assert_eq!(sweep["path_sketch"], h.0[3].1["name"]);
        let r = 20.0 / 30f64.to_radians().cos();
        let want = (12.0 * 30f64.to_radians().tan() / r).to_degrees();
        assert!((out["twist_deg"].as_f64().unwrap() - want).abs() < 1e-9);
        assert_eq!(sweep["twist"], json!(format!("{want:.6} deg")));

        let mut h = Fake::default();
        let out = build(&mut h, Kind::Helical, &json!({"hand": "left", "plane": "YZ", "thickness": 12})).unwrap();
        assert!(out["twist_deg"].as_f64().unwrap() < 0.0);
        assert_eq!(h.0[4].1["points"], json!([[0, 0], [12.0, 0.0]]));
    }

    #[test]
    fn helical_refuses_bad_helix_and_hand() {
        for p in [json!({"helix_angle": 0}), json!({"helix_angle": 50}), json!({"hand": "up"})] {
            let mut h = Fake::default();
            assert!(build(&mut h, Kind::Helical, &p).is_err(), "{p}");
            assert!(h.0.is_empty());
        }
    }
}
