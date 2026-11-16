//! Runs the packaged gears plugin inside SolidCraft's real Wasm runtime with a
//! recording host, so manifest parsing, sandbox limits and host calls are exercised.
use craft_plugin::{CommandSchema, Effect, FileGrant, PluginHost, Result};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Default)]
struct Recorder { calls: Vec<(String, Value)>, log: Vec<String>, committed: bool, rolled_back: bool }

impl PluginHost for Recorder {
    fn command_catalog(&self) -> Vec<CommandSchema> {
        ["sketch.create", "sketch.line", "sketch.circle.center", "sketch.finish", "solid.extrude"]
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
        ("defaults", json!({})),
        ("150 teeth, bore", json!({"teeth": 150, "module": 1, "bore": 20, "flank_points": 12})),
        ("bad: 3 teeth", json!({"teeth": 3})),
    ];
    for (label, params) in cases {
        let (host, result) = craft_plugin_wasm::run(Recorder::default(), &manifest, &wasm, "spur", &params);
        let summary: Vec<String> = host.calls.iter().map(|(c, a)| match a.get("points") {
            Some(p) => format!("{c}({} pts)", p.as_array().map_or(0, |v| v.len())),
            None => c.clone(),
        }).collect();
        println!("== {label}: committed={} rolled_back={}", host.committed, host.rolled_back);
        println!("   calls: {}", summary.join(" -> "));
        match result {
            Ok(v) => println!("   result: {}", serde_json::to_string(&v).unwrap()),
            Err(e) => println!("   error: {e:?}"),
        }
    }
    let (_, r) = craft_plugin_wasm::run(Recorder::default(), &manifest, &wasm, "nope", &json!({}));
    println!("== unknown entry: {:?}", r.err());
}
