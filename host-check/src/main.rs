//! Runs the packaged gears plugin inside SolidCraft's real Wasm runtime with a
//! recording host, so manifest parsing, sandbox limits and host calls are exercised.
use craft_plugin::{CommandSchema, Effect, FileGrant, PluginHost, Result};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Default)]
struct Recorder { calls: Vec<(String, Value)>, log: Vec<String>, committed: bool, rolled_back: bool }

impl PluginHost for Recorder {
    fn command_catalog(&self) -> Vec<CommandSchema> {
        ["sketch.create", "sketch.line", "sketch.circle.center", "sketch.finish", "solid.extrude", "solid.sweep"]
            .iter()
            .map(|n| CommandSchema { name: n.to_string(), input_schema: json!({"type":"object"}), output_schema: json!({}), effect: Effect::Edit, group: None })
            .collect()
    }
    fn run_command(&mut self, name: &str, args: &Value) -> Result<Value> { self.calls.push((name.into(), args.clone())); Ok(json!({"ok": true})) }
    fn begin_transaction(&mut self) -> Result<()> { Ok(()) }
    fn commit_transaction(&mut self) -> Result<()> { self.committed = true; Ok(()) }
    fn rollback_transaction(&mut self) -> Result<()> { self.rolled_back = true; Ok(()) }
    fn granted_folders(&self) -> Vec<FileGrant> { vec![] }
    fn log(&mut self, m: &str) { self.log.push(m.into()) }
    fn progress(&mut self, _: f32, _: &str) {}
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../package");
    let manifest_bytes = std::fs::read(dir.join("plugin.json")).expect("manifest");
    let manifest = craft_plugin::parse_manifest(&manifest_bytes, "gears").expect("manifest parses");
    let wasm = std::fs::read(dir.join("gears.wasm")).expect("component");
    let cases = [
        ("spur", "defaults", json!({})),
        ("spur", "bad: 3 teeth", json!({"teeth": 3})),
        ("helical", "LH 30deg on YZ, bore", json!({"helix_angle": 30, "hand": "left", "plane": "YZ", "bore": 10, "thickness": 20})),
        ("herringbone", "defaults, bore 8", json!({"bore": 8})),
        ("bevel", "20:40, bore 8", json!({"teeth": 20, "mate_teeth": 40, "bore": 8})),
        ("bevel", "bad: face too wide", json!({"face_width": 500})),
        ("worm", "1 start, defaults", json!({})),
        ("worm", "2 starts, 30 turns", json!({"starts": 2, "length": 377})),
        ("worm-wheel", "30 teeth for 1-start worm", json!({"teeth": 30})),
        ("rack", "defaults (with 16-tooth pinion)", json!({})),
        ("rack", "500 teeth, pinion bore 8", json!({"teeth": 500, "pinion_teeth": 24, "pinion_bore": 8})),
        ("rack", "bad: pinion bore too big", json!({"pinion_bore": 100})),
        ("hypoid", "gear, bore 12", json!({"member": "gear", "bore": 12})),
        ("hypoid", "pinion", json!({"member": "pinion"})),
        ("hypoid", "bad: offset 40", json!({"offset": 40})),
        ("internal", "60 teeth", json!({})),
        ("internal", "300 teeth", json!({"teeth": 300, "module": 1})),
        ("planetary", "12/18/48 x3, bores", json!({"sun_bore": 5, "planet_bore": 4})),
        ("planetary", "24/12/48 x12? (collide)", json!({"sun_teeth": 24, "planet_teeth": 12, "planets": 12})),
        ("planetary", "30/15/60 x5", json!({"sun_teeth": 30, "planet_teeth": 15, "planets": 5})),
        ("cycloidal", "60:10 pair", json!({})),
        ("cycloidal", "bad: 3 teeth", json!({"mate_teeth": 3})),
        ("cycloidal-drive", "defaults", json!({})),
        ("cycloidal-drive", "bore 10, 6 output holes", json!({"disc_bore": 10, "output_holes": 6, "output_pin_diameter": 4, "output_circle_diameter": 34})),
        ("cycloidal-drive", "bad: eccentricity 10", json!({"eccentricity": 10})),
    ];
    for (entry, label, params) in cases {
        let label = format!("{entry} {label}");
        let (host, result) = craft_plugin_wasm::run(Recorder::default(), &manifest, &wasm, entry, &params);
        let mut summary: Vec<String> = host.calls.iter().map(|(c, a)| match a.get("points") {
            Some(p) => format!("{c}({} pts)", p.as_array().map_or(0, |v| v.len())),
            None if c == "solid.sweep" => format!("{c}({} {})", a["operation"], a.get("twist").or(a.get("scale")).unwrap_or(&json!(""))),
            None => c.clone(),
        }).collect();
        println!("== {label}: committed={} rolled_back={}", host.committed, host.rolled_back);
        summary.dedup();
        println!("   calls ({}): {}", host.calls.len(), summary.join(" -> "));
        match result {
            Ok(v) => {
                let keep = ["body", "bodies", "ratio_ring_fixed", "ratio", "pitch_diameter_mm", "twist_deg", "twist_each_half_deg", "pitch_angle_deg", "apex_distance_mm", "lead_mm", "sections", "centre_distance_mm", "helix_angle_deg", "hand", "offset_angle_deg"];
                let short: serde_json::Map<String, Value> = v.as_object().unwrap().iter().filter(|(k, _)| keep.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
                println!("   result: {}", Value::Object(short));
            }
            Err(e) => println!("   error: {e:?}"),
        }
    }
    let (_, r) = craft_plugin_wasm::run(Recorder::default(), &manifest, &wasm, "nope", &json!({}));
    println!("== unknown entry: {:?}", r.err());
}
