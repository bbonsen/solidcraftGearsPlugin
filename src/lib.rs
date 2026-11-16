//! SolidCraft gear plugin: spur, helical, herringbone, straight bevel and worm gears, plus a
//! wheel to mate with a worm. Each entry computes a tooth profile and builds it with native
//! sketches, extrudes and sweeps, so the result is an ordinary editable feature.

pub mod gear;

use gear::{BevelGear, CycloidalDrive, CycloidalGear, Hypoid, InternalGear, Mods, Planetary, Rack, SpurGear, Worm};
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
    Herringbone,
    Bevel,
    Worm,
    WormWheel,
    Rack,
    Hypoid,
    Internal,
    Planetary,
    Cycloidal,
    CycloidalDrive,
}

impl Kind {
    pub fn from_entry(entry: &str) -> Option<Kind> {
        match entry {
            "spur" => Some(Kind::Spur),
            "helical" => Some(Kind::Helical),
            "herringbone" => Some(Kind::Herringbone),
            "bevel" => Some(Kind::Bevel),
            "worm" => Some(Kind::Worm),
            "worm-wheel" => Some(Kind::WormWheel),
            "rack" => Some(Kind::Rack),
            "hypoid" => Some(Kind::Hypoid),
            "internal" => Some(Kind::Internal),
            "planetary" => Some(Kind::Planetary),
            "cycloidal" => Some(Kind::Cycloidal),
            "cycloidal-drive" => Some(Kind::CycloidalDrive),
            _ => None,
        }
    }
}

/// Where the point `d` mm along `plane`'s normal from the origin lies in the path sketch, and
/// which plane that sketch is on. Sketch axes: XY → (+X, +Y), XZ → (+X, -Z), YZ → (+Y, +Z).
fn path_plane(plane: &str) -> &'static str {
    match plane {
        "XY" => "XZ",
        _ => "XY",
    }
}

fn axis_point(plane: &str, d: f64) -> Value {
    let d = round(d);
    match plane {
        "XZ" => json!([0.0, d]), // normal +Y = XY sketch +y
        "YZ" => json!([d, 0.0]), // normal +X = XY sketch +x
        _ => json!([0.0, -d]),   // normal +Z = XZ sketch -y
    }
}

/// The inputs every entry shares.
struct Common {
    plane: &'static str,
    bore: f64,
    backlash: f64,
    flank_points: u32,
}

fn common(params: &Value) -> Result<Common, Failure> {
    let plane = match params.get("plane").and_then(Value::as_str).unwrap_or("XY") {
        "XY" => "XY",
        "XZ" => "XZ",
        "YZ" => "YZ",
        _ => return Err(Failure::Input("plane", "must be XY, XZ or YZ".into())),
    };
    Ok(Common {
        plane,
        bore: num(params, "bore", 0.0)?,
        backlash: num(params, "backlash", 0.0)?,
        flank_points: int(params, "flank_points", 8)?,
    })
}

fn length(params: &Value, key: &'static str, default: f64) -> Result<f64, Failure> {
    let v = num(params, key, default)?;
    if v > 0.0 && v <= 10_000.0 {
        Ok(v)
    } else {
        Err(Failure::Input(key, "must be more than 0 and at most 10000 mm".into()))
    }
}

/// True for right hand.
fn hand(params: &Value) -> Result<bool, Failure> {
    match params.get("hand").and_then(Value::as_str).unwrap_or("right") {
        "right" => Ok(true),
        "left" => Ok(false),
        _ => Err(Failure::Input("hand", "must be right or left".into())),
    }
}

fn check_bore(bore: f64, root_radius: f64) -> Result<(), Failure> {
    let max = 2.0 * root_radius * 0.9;
    if bore < 0.0 || bore >= max {
        return Err(Failure::Input("bore", format!("must be 0 (none) or less than {max:.3} mm")));
    }
    Ok(())
}

fn profile_err((f, r): (&'static str, String)) -> Failure {
    Failure::Input(f, r)
}

/// Sketch a closed profile (and optional bore at the origin) on `plane`, `offset` mm along its normal.
fn sketch_profile(host: &mut dyn Host, plane: &str, offset: f64, name: &str, pts: &[[f64; 2]], bore: f64) -> Result<(), Failure> {
    sketch_profile_at(host, plane, offset, name, pts, bore, [0.0, 0.0])
}

fn sketch_profile_at(
    host: &mut dyn Host,
    plane: &str,
    offset: f64,
    name: &str,
    pts: &[[f64; 2]],
    bore: f64,
    centre: [f64; 2],
) -> Result<(), Failure> {
    let mut args = json!({"plane": plane, "name": name});
    if offset != 0.0 {
        args["offset"] = json!(round(offset));
    }
    host.run("sketch.create", args)?;
    let pts: Vec<Value> = pts.iter().map(|p| json!([round(p[0]), round(p[1])])).collect();
    host.run("sketch.line", json!({"points": pts, "closed": true}))?;
    if bore > 0.0 {
        host.run("sketch.circle.center", json!({"center": [round(centre[0]), round(centre[1])], "diameter": bore}))?;
    }
    host.run("sketch.finish", json!({}))?;
    if host.cancelled() {
        return Err(Failure::Cancelled);
    }
    Ok(())
}

/// One sketch of straight path segments along the axis, each from `a` to `b` mm, named `id`.
fn sketch_axis(host: &mut dyn Host, plane: &str, name: &str, segments: &[(String, f64, f64)]) -> Result<(), Failure> {
    host.run("sketch.create", json!({"plane": path_plane(plane), "name": name}))?;
    for (id, a, b) in segments {
        host.run("sketch.line", json!({"points": [axis_point(plane, *a), axis_point(plane, *b)], "ids": [id]}))?;
    }
    host.run("sketch.finish", json!({}))?;
    Ok(())
}

/// Sweep `sketch` along `path` of `path_sketch`. The first sweep makes the body; later ones join it.
fn sweep(host: &mut dyn Host, sketch: &str, path_sketch: &str, path: &str, body: &str, first: bool, extra: Value) -> Result<(), Failure> {
    let mut args = json!({"sketch": sketch, "path_sketch": path_sketch, "path": [path], "body_name": body});
    if first {
        args["operation"] = json!("new");
    } else {
        args["operation"] = json!("join");
        args["targets"] = json!([body]);
    }
    if let (Some(a), Some(e)) = (args.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            a.insert(k.clone(), v.clone());
        }
    }
    host.run("solid.sweep", args)?;
    Ok(())
}

fn deg(x: f64) -> String {
    format!("{x:.6} deg")
}

/// Tooth modifications from the entry's inputs.
fn mods(params: &Value) -> Result<Mods, Failure> {
    Ok(Mods {
        shift: num(params, "profile_shift", 0.0)?,
        root_fillet: num(params, "root_fillet", 0.0)?,
        tip_relief: num(params, "tip_relief", 0.0)?,
        relief_length: num(params, "relief_length", 0.0)?,
    })
}

fn cylindrical_gear(params: &Value, c: &Common, helix_angle: f64) -> Result<SpurGear, Failure> {
    Ok(SpurGear {
        teeth: int(params, "teeth", 20)?,
        module: num(params, "module", 2.0)?,
        pressure_angle: num(params, "pressure_angle", 20.0)?,
        helix_angle,
        backlash: c.backlash,
        flank_points: c.flank_points,
        mods: mods(params)?,
    })
}

fn helix(params: &Value) -> Result<f64, Failure> {
    let beta = num(params, "helix_angle", 20.0)?;
    if beta.is_finite() && beta > 0.0 && beta <= 45.0 {
        Ok(beta)
    } else {
        Err(Failure::Input("helix_angle", "must be more than 0 and at most 45 degrees".into()))
    }
}

/// Facts about a spur, helical or herringbone gear (or worm wheel) for the reply.
fn describe(g: &SpurGear, name: &str) -> Value {
    let c = g.circles();
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
        "warnings": if undercut {
            vec![format!(
                "{} teeth is below {:.1}: a real cut gear would be undercut; this profile is not. A profile shift of {:.3} avoids it",
                g.teeth,
                g.min_teeth_without_undercut(),
                g.shift_to_avoid_undercut()
            )]
        } else { vec![] },
    });
    if g.mods != Mods::default() {
        out["profile_shift"] = json!(g.mods.shift);
        out["root_fillet_mm"] = json!(g.mods.root_fillet * g.module);
        out["tip_relief_mm"] = json!(g.mods.tip_relief);
    }
    if g.mods.shift != 0.0 {
        out["centre_distance_note"] = json!(
            "with profile shift, a mate whose shift is the opposite (x2 = -x1) keeps the standard centre distance (z1 + z2) * transverse module / 2; otherwise the gears sit further apart: inv(aw) = inv(at) + 2 tan(an) (x1 + x2) / (z1 + z2), a = a0 cos(at) / cos(aw)"
        );
    }
    if g.helix_angle > 0.0 {
        out["helix_angle_deg"] = json!(g.helix_angle);
        out["transverse_module_mm"] = json!(g.transverse_module());
        out["transverse_pressure_angle_deg"] = json!(g.transverse_pressure_angle().to_degrees());
        out["lead_mm"] = json!(PI * 2.0 * c.pitch / g.helix_angle.to_radians().tan());
    }
    out
}

/// Sections in a crowned spur's loft (ends, mid-width and between).
const CROWN_SECTIONS: usize = 7;

/// Extra thinning of a crowned tooth (total, both flanks) at height `z` of face width `b`.
fn crown_thinning(c: f64, z: f64, b: f64) -> f64 {
    let t = 2.0 * z / b - 1.0;
    2.0 * c * t * t
}

/// Build a gear. `params` is the entry's JSON input.
pub fn build(host: &mut dyn Host, kind: Kind, params: &Value) -> Result<Value, Failure> {
    let c = common(params)?;
    match kind {
        Kind::Spur | Kind::Helical | Kind::WormWheel => {
            let (g, right, mut extra) = if kind == Kind::WormWheel {
                let w = worm(params, &c)?;
                let right = hand(params)?;
                let g = w.wheel(int(params, "teeth", 30)?);
                let wc = w.circles();
                let centre = wc.pitch + g.circles().pitch;
                let ratio = f64::from(g.teeth) / f64::from(w.starts);
                (g, right, json!({
                    "worm_pitch_diameter_mm": 2.0 * wc.pitch,
                    "centre_distance_mm": centre,
                    "ratio": ratio,
                    "mating_note": "the worm's axis crosses this wheel's axis at 90 degrees, centre_distance_mm away; use the same module, starts, diameter factor, pressure angle and hand",
                    "approximation": "a helical wheel, not throated: line contact becomes point contact, fine for light loads and printing",
                }))
            } else if kind == Kind::Helical {
                let beta = helix(params)?;
                (cylindrical_gear(params, &c, beta)?, hand(params)?, json!({
                    "mating_note": "a mating gear on a parallel shaft needs the same helix angle and the opposite hand",
                }))
            } else {
                (cylindrical_gear(params, &c, 0.0)?, true, json!({}))
            };
            let thickness = length(params, "thickness", 10.0)?;
            let crown0 = if kind == Kind::Spur { num(params, "crowning", 0.0)? } else { 0.0 };
            if !(crown0.is_finite() && crown0 >= 0.0 && crown0 <= 0.1 * g.module) {
                return Err(Failure::Input("crowning", format!("must be 0..={:.3} mm (0.1 × module)", 0.1 * g.module)));
            }
            let first = SpurGear { backlash: g.backlash + crown_thinning(crown0, 0.0, thickness), ..g.clone() };
            let pts = first.profile().map_err(profile_err)?;
            check_bore(c.bore, g.circles().root)?;
            let name = match kind {
                Kind::Spur => format!("Spur gear z{} m{}", g.teeth, g.module),
                Kind::Helical => format!("Helical gear z{} m{} {}{}", g.teeth, g.module, if right { "RH" } else { "LH" }, g.helix_angle),
                _ => format!("Worm wheel z{} m{} {}", g.teeth, round(g.transverse_module()), if right { "RH" } else { "LH" }),
            };
            host.progress(0.1, "sketching profile");
            sketch_profile(host, c.plane, 0.0, &name, &pts, c.bore)?;
            let mut out = describe(&g, &name);
            let crowning = crown0;
            if kind == Kind::Spur && crowning > 0.0 {
                // Crowned teeth vary along the face: loft through sections whose teeth are thinned
                // by 2·c·(2z/b − 1)², c at each face, nothing at mid-width.
                let mut sections = vec![json!({"sketch": name})];
                for k in 1..CROWN_SECTIONS {
                    if host.cancelled() {
                        return Err(Failure::Cancelled);
                    }
                    let z = thickness * k as f64 / (CROWN_SECTIONS - 1) as f64;
                    let sk = format!("{name} section {}", k + 1);
                    let section = SpurGear { backlash: g.backlash + crown_thinning(crowning, z, thickness), ..g.clone() };
                    let pts = section.profile().map_err(profile_err)?;
                    host.progress(0.1 + 0.6 * k as f32 / CROWN_SECTIONS as f32, "sketching crowned sections");
                    sketch_profile(host, c.plane, z, &sk, &pts, c.bore)?;
                    sections.push(json!({"sketch": sk}));
                }
                host.progress(0.8, "lofting");
                host.run("solid.loft", json!({"sections": sections, "operation": "new", "body_name": name}))?;
            } else if kind == Kind::Spur {
                host.progress(0.6, "extruding");
                host.run("solid.extrude", json!({"distance": thickness, "operation": "new", "body_name": name}))?;
            } else {
                // Right hand: the teeth turn right-handed about the axis as they advance along it,
                // which is the sweep's positive twist (right hand about the direction of travel).
                let twist = g.twist(thickness).to_degrees() * if right { 1.0 } else { -1.0 };
                let axis = format!("{name} axis");
                sketch_axis(host, c.plane, &axis, &[("axis".into(), 0.0, thickness)])?;
                host.progress(0.5, "sweeping teeth");
                sweep(host, &name, &axis, "axis", &name, true, json!({"twist": deg(twist)}))?;
                out["hand"] = json!(if right { "right" } else { "left" });
                out["twist_deg"] = json!(twist);
            }
            out["profile_points"] = json!(pts.len());
            if crown0 > 0.0 {
                out["crowning_mm"] = json!(crown0);
                out["crowning_note"] = json!("each flank is thinned by crowning_mm at both faces, parabolically to nothing at mid-width, lofted through 7 sections");
            }
            merge(&mut out, extra.take());
            host.progress(1.0, "done");
            Ok(out)
        }
        Kind::Herringbone => {
            let beta = helix(params)?;
            let g = cylindrical_gear(params, &c, beta)?;
            let right = hand(params)?;
            let thickness = length(params, "thickness", 20.0)?;
            let pts = g.profile().map_err(profile_err)?;
            check_bore(c.bore, g.circles().root)?;
            let name = format!("Herringbone gear z{} m{} {}deg", g.teeth, g.module, g.helix_angle);
            let half = thickness / 2.0;
            // Both halves start from the same profile at mid-width and sweep outwards. Equal
            // twists about their own directions of travel would make one helix; opposite twists
            // make the V. `hand` is the hand of the half on the +normal side.
            let twist = g.twist(half).to_degrees() * if right { 1.0 } else { -1.0 };
            let (upper, lower, axis) = (name.clone(), format!("{name} lower"), format!("{name} axis"));
            host.progress(0.1, "sketching profiles");
            sketch_profile(host, c.plane, half, &upper, &pts, c.bore)?;
            sketch_profile(host, c.plane, half, &lower, &pts, c.bore)?;
            sketch_axis(host, c.plane, &axis, &[("up".into(), half, thickness), ("down".into(), half, 0.0)])?;
            host.progress(0.4, "sweeping upper half");
            sweep(host, &upper, &axis, "up", &name, true, json!({"twist": deg(twist)}))?;
            if host.cancelled() {
                return Err(Failure::Cancelled);
            }
            host.progress(0.7, "sweeping lower half");
            sweep(host, &lower, &axis, "down", &name, false, json!({"twist": deg(-twist)}))?;
            host.progress(1.0, "done");
            let mut out = describe(&g, &name);
            merge(&mut out, json!({
                "upper_hand": if right { "right" } else { "left" },
                "twist_each_half_deg": twist.abs(),
                "profile_points": pts.len(),
                "mating_note": "a mating herringbone gear needs the same helix angle and the opposite hand; herringbone gears need no thrust bearing because the halves cancel",
            }));
            Ok(out)
        }
        Kind::Bevel => {
            let b = BevelGear {
                teeth: int(params, "teeth", 20)?,
                mate_teeth: int(params, "mate_teeth", 20)?,
                module: num(params, "module", 2.0)?,
                pressure_angle: num(params, "pressure_angle", 20.0)?,
                backlash: c.backlash,
                flank_points: c.flank_points,
                face_width: num(params, "face_width", 0.0)?,
            };
            let name = format!("Bevel gear z{}:{} m{}", b.teeth, b.mate_teeth, b.module);
            let pts = cone_gear(host, &c, &b, &name, 0.0)?;
            let delta = b.pitch_angle();
            let len = b.axial_length();
            host.progress(1.0, "done");
            Ok(json!({
                "body": name,
                "teeth": b.teeth,
                "mate_teeth": b.mate_teeth,
                "module_mm": b.module,
                "pressure_angle_deg": b.pressure_angle,
                "pitch_angle_deg": delta.to_degrees(),
                "pitch_diameter_mm": 2.0 * b.pitch_radius(),
                "outside_diameter_mm": 2.0 * (b.pitch_radius() + b.module * delta.cos()),
                "cone_distance_mm": b.cone_distance(),
                "face_width_mm": b.face(),
                "axial_length_mm": len,
                "apex_distance_mm": b.apex_distance(),
                "profile_points": pts,
                "mating_note": "make the mate with teeth and mate_teeth swapped; the two axes meet at 90 degrees at the cone apex, apex_distance_mm along each gear's axis from its heel plane",
                "approximation": "Tredgold's back-cone approximation, the usual way straight bevel teeth are drawn",
            }))
        }
        Kind::Rack => {
            let module = num(params, "module", 2.0)?;
            let r = Rack {
                teeth: int(params, "teeth", 20)?,
                module,
                pressure_angle: num(params, "pressure_angle", 20.0)?,
                backlash: c.backlash,
                back: num(params, "back", 2.5 * module)?,
            };
            let thickness = length(params, "thickness", 10.0)?;
            let pinion_teeth = int(params, "pinion_teeth", 16)?;
            let rack_pts = r.profile().map_err(profile_err)?;
            // Check the pinion before any command runs.
            let pinion = if pinion_teeth > 0 {
                let g = r.pinion_with(pinion_teeth, c.flank_points, mods(params)?);
                let (pts, centre) = r.pinion_in_mesh(&g, r.teeth / 2).map_err(profile_err)?;
                let bore = num(params, "pinion_bore", 0.0)?;
                check_bore(bore, g.circles().root).map_err(|_| {
                    Failure::Input("pinion_bore", format!("must be 0 (none) or less than {:.3} mm", 1.8 * g.circles().root))
                })?;
                Some((g, pts, centre, bore))
            } else {
                None
            };
            let name = format!("Rack m{} x{}", r.module, r.teeth);
            host.progress(0.1, "sketching rack");
            sketch_profile(host, c.plane, 0.0, &name, &rack_pts, 0.0)?;
            host.run("solid.extrude", json!({"sketch": name, "distance": thickness, "operation": "new", "body_name": name}))?;
            let mut out = json!({
                "body": name,
                "teeth": r.teeth,
                "module_mm": r.module,
                "pressure_angle_deg": r.pressure_angle,
                "circular_pitch_mm": r.pitch(),
                "length_mm": r.length(),
                "pitch_line": "sketch y = 0; tips at y = module, root at y = -1.25 module, back at y = -1.25 module - back",
                "rack_points": rack_pts.len(),
            });
            if let Some((g, pts, centre, bore)) = pinion {
                let pname = format!("Pinion z{} m{}", g.teeth, g.module);
                host.progress(0.5, "sketching pinion");
                sketch_profile_at(host, c.plane, 0.0, &pname, &pts, bore, centre)?;
                host.run("solid.extrude", json!({"sketch": pname, "distance": thickness, "operation": "new", "body_name": pname}))?;
                let mut p = describe(&g, &pname);
                if let Some(o) = p.as_object_mut() {
                    o.remove("centre_distance_note");
                }
                p["centre_in_sketch_mm"] = json!([centre[0], centre[1]]);
                p["rack_travel_per_turn_mm"] = json!(r.pitch() * f64::from(g.teeth));
                p["profile_points"] = json!(pts.len());
                out["pinion"] = p;
                out["mating_note"] = json!("the pinion's pitch circle touches the rack's pitch line; one pinion turn moves the rack rack_travel_per_turn_mm");
            }
            host.progress(1.0, "done");
            Ok(out)
        }
        Kind::Hypoid => {
            let h = Hypoid {
                gear_teeth: int(params, "gear_teeth", 37)?,
                pinion_teeth: int(params, "pinion_teeth", 11)?,
                module: num(params, "module", 2.0)?,
                pressure_angle: num(params, "pressure_angle", 20.0)?,
                spiral_angle: num(params, "spiral_angle", 30.0)?,
                offset: num(params, "offset", 8.0)?,
                backlash: c.backlash,
                flank_points: c.flank_points,
                face_width: num(params, "face_width", 0.0)?,
            };
            let build_pinion = match params.get("member").and_then(Value::as_str).unwrap_or("gear") {
                "gear" => false,
                "pinion" => true,
                _ => return Err(Failure::Input("member", "must be gear or pinion".into())),
            };
            // `hand` is the pinion's; the gear takes the other hand.
            let pinion_right = match params.get("hand").and_then(Value::as_str).unwrap_or("left") {
                "right" => true,
                "left" => false,
                _ => return Err(Failure::Input("hand", "must be right or left".into())),
            };
            let (gear, pinion) = h.members().map_err(profile_err)?;
            let (m, right) = if build_pinion { (&pinion, pinion_right) } else { (&gear, !pinion_right) };
            let twist = m.twist.to_degrees() * if right { 1.0 } else { -1.0 };
            let name = format!(
                "Hypoid {} z{}:{} m{} E{} {}",
                if build_pinion { "pinion" } else { "gear" },
                h.gear_teeth,
                h.pinion_teeth,
                h.module,
                h.offset,
                if right { "RH" } else { "LH" }
            );
            let pts = cone_gear(host, &c, &m.bevel, &name, twist)?;
            host.progress(1.0, "done");
            let facts = |mm: &gear::HypoidMember| {
                json!({
                    "teeth": mm.bevel.teeth,
                    "heel_module_mm": mm.bevel.module,
                    "spiral_angle_deg": mm.spiral.to_degrees(),
                    "pitch_angle_deg": mm.bevel.pitch_angle().to_degrees(),
                    "pitch_diameter_mm": 2.0 * mm.bevel.pitch_radius(),
                    "outside_diameter_mm": 2.0 * (mm.bevel.pitch_radius() + mm.bevel.module * mm.bevel.pitch_angle().cos()),
                    "cone_distance_mm": mm.bevel.cone_distance(),
                    "axial_length_mm": mm.bevel.axial_length(),
                    "apex_distance_mm": mm.bevel.apex_distance(),
                })
            };
            Ok(json!({
                "body": name,
                "member": if build_pinion { "pinion" } else { "gear" },
                "hand": if right { "right" } else { "left" },
                "twist_deg": twist,
                "offset_mm": h.offset,
                "offset_angle_deg": h.offset_angle().to_degrees(),
                "face_width_mm": m.bevel.face(),
                "ratio": f64::from(h.gear_teeth) / f64::from(h.pinion_teeth),
                "gear": facts(&gear),
                "pinion": facts(&pinion),
                "profile_points": pts,
                "mating_note": "run the entry again with the other member and the same inputs; the axes cross at 90 degrees, offset_mm apart, each apex about apex_distance_mm from its heel plane",
                "approximation": "spiral bevel members sized by first-order hypoid rules (offset angle, pinion spiral angle, enlarged pinion); pitch cones are those of a bevel pair and the flanks are not machine-generated, so expect to tune backlash and placement",
            }))
        }
        Kind::Internal => {
            let module = num(params, "module", 2.0)?;
            let g = InternalGear {
                teeth: int(params, "teeth", 60)?,
                module,
                pressure_angle: num(params, "pressure_angle", 20.0)?,
                backlash: c.backlash,
                flank_points: c.flank_points,
                rim: num(params, "rim", 3.0 * module)?,
            };
            let thickness = length(params, "thickness", 10.0)?;
            let pinion_teeth = int(params, "pinion_teeth", 0)?;
            let pts = g.profile(0.0).map_err(profile_err)?;
            let gc = g.circles();
            let name = format!("Internal gear z{} m{}", g.teeth, g.module);
            host.progress(0.1, "sketching ring");
            // The outer circle and the toothed inner outline: the region between them is the ring.
            sketch_profile(host, c.plane, 0.0, &name, &pts, 2.0 * g.outer_radius())?;
            host.progress(0.6, "extruding");
            host.run("solid.extrude", json!({"sketch": name, "distance": thickness, "operation": "new", "body_name": name}))?;
            host.progress(1.0, "done");
            let mut warnings = Vec::new();
            if pinion_teeth > 0 && g.teeth < pinion_teeth + g.min_difference() {
                warnings.push(format!(
                    "a {pinion_teeth}-tooth pinion is within {} teeth of this ring: expect tip interference",
                    g.min_difference()
                ));
            }
            if gc.tip < gc.base {
                warnings.push("the ring's tooth tips lie inside the base circle, so their last part is radial, not involute".to_string());
            }
            Ok(json!({
                "body": name,
                "teeth": g.teeth,
                "module_mm": g.module,
                "pressure_angle_deg": g.pressure_angle,
                "pitch_diameter_mm": 2.0 * gc.pitch,
                "inner_diameter_mm": 2.0 * gc.tip,
                "root_diameter_mm": 2.0 * gc.root,
                "outer_diameter_mm": 2.0 * g.outer_radius(),
                "profile_points": pts.len(),
                "mating_note": "a spur pinion with the same module and pressure angle meshes inside, its centre (ring teeth - pinion teeth) * module / 2 from the ring's centre",
                "warnings": warnings,
            }))
        }
        Kind::Planetary => {
            let module = num(params, "module", 2.0)?;
            let p = Planetary {
                sun_teeth: int(params, "sun_teeth", 12)?,
                planet_teeth: int(params, "planet_teeth", 18)?,
                planets: int(params, "planets", 3)?,
                module,
                pressure_angle: num(params, "pressure_angle", 20.0)?,
                backlash: c.backlash,
                flank_points: c.flank_points,
                rim: num(params, "rim", 3.0 * module)?,
                mods: Mods { shift: 0.0, ..mods(params)? },
            };
            let thickness = length(params, "thickness", 10.0)?;
            let ((sun, _), planets, (ring, _)) = p.layout().map_err(profile_err)?;
            let sun_bore = num(params, "sun_bore", 0.0)?;
            check_bore(sun_bore, p.carrier_radius() - module * f64::from(p.planet_teeth) / 2.0 - 1.25 * module)
                .map_err(|f| match f { Failure::Input(_, r) => Failure::Input("sun_bore", r), other => other })?;
            let planet_bore = num(params, "planet_bore", 0.0)?;
            check_bore(planet_bore, module * f64::from(p.planet_teeth) / 2.0 - 1.25 * module)
                .map_err(|f| match f { Failure::Input(_, r) => Failure::Input("planet_bore", r), other => other })?;

            let z = p.ring_teeth();
            let tag = format!("z{}/{}/{} m{}", p.sun_teeth, p.planet_teeth, z, module);
            let mut bodies = Vec::new();
            let mut extrude = |host: &mut dyn Host, name: &str| -> Result<(), Failure> {
                host.run("solid.extrude", json!({"sketch": name, "distance": thickness, "operation": "new", "body_name": name}))?;
                bodies.push(name.to_string());
                Ok(())
            };
            host.progress(0.05, "sun");
            let sun_name = format!("Sun {tag}");
            sketch_profile(host, c.plane, 0.0, &sun_name, &sun, sun_bore)?;
            extrude(host, &sun_name)?;
            let mut centres = Vec::new();
            for (i, (pts, centre)) in planets.iter().enumerate() {
                host.progress(0.1 + 0.6 * i as f32 / planets.len() as f32, "planets");
                let name = format!("Planet {} {tag}", i + 1);
                sketch_profile_at(host, c.plane, 0.0, &name, pts, planet_bore, *centre)?;
                extrude(host, &name)?;
                centres.push(json!([round(centre[0]), round(centre[1])]));
            }
            host.progress(0.8, "ring");
            let ring_name = format!("Ring {tag}");
            sketch_profile(host, c.plane, 0.0, &ring_name, &ring, 2.0 * p.ring().outer_radius())?;
            extrude(host, &ring_name)?;
            host.progress(1.0, "done");
            let limit = 2.0 / p.pressure_angle.to_radians().sin().powi(2);
            let warnings: Vec<String> = [p.sun_teeth, p.planet_teeth]
                .iter()
                .filter(|&&t| f64::from(t) < limit)
                .map(|t| format!("{t} teeth is below the undercut limit ({limit:.1}): real cut gears would be undercut; these profiles are not"))
                .collect();
            Ok(json!({
                "bodies": bodies,
                "sun_teeth": p.sun_teeth,
                "planet_teeth": p.planet_teeth,
                "ring_teeth": z,
                "planets": p.planets,
                "module_mm": module,
                "carrier_radius_mm": p.carrier_radius(),
                "planet_centres_mm": centres,
                "ratio_ring_fixed": p.ratio(),
                "ratio_note": "ring held: sun to carrier is ratio_ring_fixed : 1; carrier held: sun to ring is ring/sun teeth, reversed; sun held: ring to carrier is 1 + sun/ring teeth",
                "ring_outer_diameter_mm": 2.0 * p.ring().outer_radius(),
                "warnings": warnings,
            }))
        }
        Kind::Cycloidal => {
            let g = CycloidalGear {
                teeth: int(params, "teeth", 60)?,
                mate_teeth: int(params, "mate_teeth", 10)?,
                module: num(params, "module", 1.0)?,
                backlash: c.backlash,
                flank_points: int(params, "flank_points", 12)?,
            };
            let both = match params.get("build").and_then(Value::as_str).unwrap_or("both") {
                "both" => true,
                "gear" => false,
                _ => return Err(Failure::Input("build", "must be both or gear".into())),
            };
            let thickness = length(params, "thickness", 6.0)?;
            let ((gear_pts, _), (mate_pts, mate_centre)) = g.pair().map_err(profile_err)?;
            check_bore(c.bore, g.root_radius())?;
            let mate_bore = num(params, "mate_bore", 0.0)?;
            check_bore(mate_bore, g.mate().root_radius())
                .map_err(|f| match f { Failure::Input(_, r) => Failure::Input("mate_bore", r), other => other })?;
            let name = format!("Cycloidal gear z{} m{}", g.teeth, g.module);
            let mate_name = format!("Cycloidal gear z{} m{}", g.mate_teeth, g.module);
            let mate_name = if mate_name == name { format!("{mate_name} mate") } else { mate_name };
            host.progress(0.1, "gear");
            sketch_profile(host, c.plane, 0.0, &name, &gear_pts, c.bore)?;
            host.run("solid.extrude", json!({"sketch": name, "distance": thickness, "operation": "new", "body_name": name}))?;
            let mut bodies = vec![name.clone()];
            if both {
                host.progress(0.5, "mate");
                sketch_profile_at(host, c.plane, 0.0, &mate_name, &mate_pts, mate_bore, mate_centre)?;
                host.run("solid.extrude", json!({"sketch": mate_name, "distance": thickness, "operation": "new", "body_name": mate_name}))?;
                bodies.push(mate_name);
            }
            host.progress(1.0, "done");
            Ok(json!({
                "bodies": bodies,
                "teeth": g.teeth,
                "mate_teeth": g.mate_teeth,
                "module_mm": g.module,
                "pitch_diameter_mm": 2.0 * g.pitch_radius(),
                "tip_diameter_mm": 2.0 * g.tip_radius(),
                "root_diameter_mm": 2.0 * g.root_radius(),
                "mate_pitch_diameter_mm": 2.0 * g.mate().pitch_radius(),
                "mate_tip_diameter_mm": 2.0 * g.mate().tip_radius(),
                "centre_distance_mm": mate_centre[0],
                "ratio": f64::from(g.teeth) / f64::from(g.mate_teeth),
                "profile_points": gear_pts.len(),
                "mating_note": "cycloidal teeth are drawn for one mate: they run correctly only with that tooth count and exactly at centre_distance_mm (the mate is placed there on +x)",
            }))
        }
        Kind::CycloidalDrive => {
            let d = CycloidalDrive {
                pins: int(params, "pins", 12)?,
                pin_circle: num(params, "pin_circle_diameter", 60.0)? / 2.0,
                pin_radius: num(params, "pin_diameter", 6.0)? / 2.0,
                eccentricity: num(params, "eccentricity", 1.5)?,
                lobe_points: int(params, "lobe_points", 60)?,
            };
            let thickness = length(params, "thickness", 8.0)?;
            let wall = length(params, "housing_wall", 5.0)?;
            let disc = d.disc().map_err(profile_err)?;
            let bore = d.housing_bore().map_err(profile_err)?;
            let e = d.eccentricity;
            // The disc's smallest radius about its own centre bounds what can be cut from it.
            let inner = disc.iter().map(|p| p[0].hypot(p[1])).fold(f64::MAX, f64::min);
            let disc_bore = num(params, "disc_bore", 0.0)?;
            if !(disc_bore >= 0.0 && disc_bore < 2.0 * inner - 4.0) {
                return Err(Failure::Input("disc_bore", format!("must be 0 (none) or less than {:.3} mm", 2.0 * inner - 4.0)));
            }
            let holes = int(params, "output_holes", 0)?;
            let out_pin = num(params, "output_pin_diameter", 5.0)?;
            let out_circle = num(params, "output_circle_diameter", (disc_bore / 2.0 + inner) / 1.0)? / 2.0;
            let hole_d = out_pin + 2.0 * e;
            if holes > 0 {
                if holes > 24 {
                    return Err(Failure::Input("output_holes", "must be 0..=24".into()));
                }
                if !(out_pin > 0.0) {
                    return Err(Failure::Input("output_pin_diameter", "must be more than 0".into()));
                }
                let fits = out_circle - hole_d / 2.0 > disc_bore / 2.0 + 1.0
                    && out_circle + hole_d / 2.0 < inner - 1.0
                    && 2.0 * out_circle * (PI / f64::from(holes)).sin() > hole_d + 1.0;
                if !fits {
                    return Err(Failure::Input(
                        "output_holes",
                        format!("{holes} holes of {hole_d:.3} mm (output pin + 2 × eccentricity) do not fit on a {:.3} mm circle in this disc", 2.0 * out_circle),
                    ));
                }
            }
            let tag = format!("{} pins", d.pins);
            let disc_name = format!("Cycloid disc {tag}");
            let housing_name = format!("Pin housing {tag}");
            host.progress(0.1, "disc");
            let disc_pts: Vec<[f64; 2]> = disc.iter().map(|p| [p[0] + e, p[1]]).collect();
            host.run("sketch.create", json!({"plane": c.plane, "name": disc_name}))?;
            let pts: Vec<Value> = disc_pts.iter().map(|p| json!([round(p[0]), round(p[1])])).collect();
            host.run("sketch.line", json!({"points": pts, "closed": true}))?;
            if disc_bore > 0.0 {
                host.run("sketch.circle.center", json!({"center": [round(e), 0.0], "diameter": disc_bore}))?;
            }
            // Output holes on a circle about the disc's own centre.
            for k in 0..holes {
                let a = 2.0 * PI * f64::from(k) / f64::from(holes);
                host.run("sketch.circle.center", json!({"center": [round(e + out_circle * a.cos()), round(out_circle * a.sin())], "diameter": round(hole_d)}))?;
            }
            host.run("sketch.finish", json!({}))?;
            if host.cancelled() {
                return Err(Failure::Cancelled);
            }
            host.run("solid.extrude", json!({"sketch": disc_name, "distance": thickness, "operation": "new", "body_name": disc_name}))?;
            host.progress(0.6, "housing");
            sketch_profile(host, c.plane, 0.0, &housing_name, &bore, 2.0 * (d.pin_circle + wall))?;
            host.run("solid.extrude", json!({"sketch": housing_name, "distance": thickness, "operation": "new", "body_name": housing_name}))?;
            host.progress(1.0, "done");
            let output = if holes > 0 {
                json!({"holes": holes, "hole_diameter_mm": hole_d, "pin_diameter_mm": out_pin, "circle_diameter_mm": 2.0 * out_circle,
                       "note": "output pins sit on a carrier centred on the drive's axis, on this circle; each orbits inside its hole"})
            } else {
                Value::Null
            };
            Ok(json!({
                "bodies": [disc_name, housing_name],
                "pins": d.pins,
                "lobes": d.pins - 1,
                "ratio": d.ratio(),
                "ratio_note": "one input (eccentric) turn moves the disc back by one lobe: ratio : 1, disc turning opposite to the input",
                "eccentricity_mm": e,
                "disc_centre_mm": [e, 0.0],
                "disc_min_radius_mm": inner,
                "pin_circle_diameter_mm": 2.0 * d.pin_circle,
                "housing_outer_diameter_mm": 2.0 * (d.pin_circle + wall),
                "output": output,
                "profile_points": disc_pts.len(),
                "assembly_note": "the disc is placed as it sits with the eccentric pointing along +x; the input shaft is on the housing's axis",
            }))
        }
        Kind::Worm => {
            let w = worm(params, &c)?;
            let right = hand(params)?;
            let wc = w.circles();
            check_bore(c.bore, wc.root)?;
            let lead = w.lead();
            let len = length(params, "length", 6.0 * PI * w.module)?;
            let turns = len / lead;
            if turns > 40.0 {
                return Err(Failure::Input("length", format!("must be at most 40 turns of the thread ({:.1} mm)", 40.0 * lead)));
            }
            let sign = if right { 1.0 } else { -1.0 };
            // A sweep twists at most four turns: build longer worms from joined sections.
            let sections = (turns / 3.5).ceil().max(1.0) as usize;
            let seg = len / sections as f64;
            let seg_twist = sign * 2.0 * PI * seg / lead;
            let name = format!("Worm m{} x{} {}", w.module, w.starts, if right { "RH" } else { "LH" });
            let axis = format!("{name} axis");
            let segments: Vec<(String, f64, f64)> =
                (0..sections).map(|k| (format!("s{k}"), seg * k as f64, seg * (k + 1) as f64)).collect();
            host.progress(0.05, "sketching thread sections");
            let mut points = 0;
            let mut profiles = Vec::with_capacity(sections);
            for k in 0..sections {
                let pts = w.section(seg_twist * k as f64).map_err(profile_err)?;
                points = pts.len();
                let sk = if k == 0 { name.clone() } else { format!("{name} section {}", k + 1) };
                sketch_profile(host, c.plane, seg * k as f64, &sk, &pts, c.bore)?;
                profiles.push(sk);
            }
            sketch_axis(host, c.plane, &axis, &segments)?;
            for (k, sk) in profiles.iter().enumerate() {
                host.progress(0.2 + 0.8 * k as f32 / sections as f32, "sweeping thread");
                sweep(host, sk, &axis, &segments[k].0, &name, k == 0, json!({"twist": deg(seg_twist.to_degrees())}))?;
                if host.cancelled() {
                    return Err(Failure::Cancelled);
                }
            }
            host.progress(1.0, "done");
            Ok(json!({
                "body": name,
                "starts": w.starts,
                "hand": if right { "right" } else { "left" },
                "axial_module_mm": w.module,
                "axial_pitch_mm": PI * w.module,
                "lead_mm": lead,
                "lead_angle_deg": w.lead_angle().to_degrees(),
                "pitch_diameter_mm": 2.0 * wc.pitch,
                "tip_diameter_mm": 2.0 * wc.tip,
                "root_diameter_mm": 2.0 * wc.root,
                "length_mm": len,
                "sections": sections,
                "profile_points": points,
                "mating_note": "make the wheel with the worm-wheel entry using the same module, starts, diameter factor, pressure angle and hand",
            }))
        }
    }
}

/// A cone gear: the heel profile swept towards the apex while it shrinks (and turns by
/// `twist_deg`, for spiral teeth); then the bore, cut through this body only. Returns the
/// profile's point count.
fn cone_gear(host: &mut dyn Host, c: &Common, b: &BevelGear, name: &str, twist_deg: f64) -> Result<usize, Failure> {
    let pts = b.profile().map_err(profile_err)?;
    let root = b.pitch_radius() - 1.25 * b.module * b.pitch_angle().cos();
    // The bore must clear the toe's root too.
    check_bore(c.bore, root * b.toe_scale())?;
    let axis = format!("{name} axis");
    let len = b.axial_length();
    host.progress(0.1, "sketching heel profile");
    sketch_profile(host, c.plane, 0.0, name, &pts, 0.0)?;
    sketch_axis(host, c.plane, &axis, &[("axis".into(), 0.0, len)])?;
    host.progress(0.4, "sweeping teeth towards the apex");
    let mut opts = json!({"scale": round(b.toe_scale()).to_string()});
    if twist_deg != 0.0 {
        opts["twist"] = json!(deg(twist_deg));
    }
    sweep(host, name, &axis, "axis", name, true, opts)?;
    if c.bore > 0.0 {
        host.progress(0.8, "boring");
        let bore = format!("{name} bore");
        host.run("sketch.create", json!({"plane": c.plane, "name": bore}))?;
        host.run("sketch.circle.center", json!({"center": [0, 0], "diameter": c.bore}))?;
        host.run("sketch.finish", json!({}))?;
        host.run("solid.extrude", json!({"sketch": bore, "distance": round(len), "operation": "cut", "targets": [name]}))?;
    }
    Ok(pts.len())
}

fn worm(params: &Value, c: &Common) -> Result<Worm, Failure> {
    let w = Worm {
        starts: int(params, "starts", 1)?,
        module: num(params, "module", 2.0)?,
        diameter_factor: num(params, "diameter_factor", 10.0)?,
        pressure_angle: num(params, "pressure_angle", 20.0)?,
        backlash: c.backlash,
        flank_points: c.flank_points,
    };
    w.validate().map_err(profile_err)?;
    Ok(w)
}

fn merge(out: &mut Value, extra: Value) {
    if let (Some(o), Value::Object(e)) = (out.as_object_mut(), extra) {
        o.extend(e);
    }
}

/// Micrometre precision keeps the JSON small.
fn round(x: f64) -> f64 {
    // `+ 0.0` turns -0.0 into 0.0.
    (x * 1e6).round() / 1e6 + 0.0
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
        assert_eq!(h.0[4].1["points"], json!([[0.0, 0.0], [0.0, -12.0]]));
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
        assert_eq!(h.0[4].1["points"], json!([[0.0, 0.0], [12.0, 0.0]]));
    }

    #[test]
    fn helical_refuses_bad_helix_and_hand() {
        for p in [json!({"helix_angle": 0}), json!({"helix_angle": 50}), json!({"hand": "up"})] {
            let mut h = Fake::default();
            assert!(build(&mut h, Kind::Helical, &p).is_err(), "{p}");
            assert!(h.0.is_empty());
        }
    }

    fn names(h: &Fake) -> Vec<&str> {
        h.0.iter().map(|(c, _)| c.as_str()).collect()
    }

    #[test]
    fn herringbone_sweeps_both_halves_out_from_the_middle_with_opposite_twists() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Herringbone, &json!({"thickness": 20, "helix_angle": 30, "bore": 6})).unwrap();
        assert_eq!(
            names(&h),
            [
                "sketch.create", "sketch.line", "sketch.circle.center", "sketch.finish",
                "sketch.create", "sketch.line", "sketch.circle.center", "sketch.finish",
                "sketch.create", "sketch.line", "sketch.line", "sketch.finish",
                "solid.sweep", "solid.sweep",
            ]
        );
        assert_eq!(h.0[0].1["offset"], json!(10.0));
        assert_eq!(h.0[4].1["offset"], json!(10.0));
        assert_eq!(h.0[9].1["points"], json!([[0.0, -10.0], [0.0, -20.0]]));
        assert_eq!(h.0[10].1["points"], json!([[0.0, -10.0], [0.0, 0.0]]));
        let (up, down) = (&h.0[12].1, &h.0[13].1);
        assert_eq!((up["operation"].clone(), down["operation"].clone()), (json!("new"), json!("join")));
        assert_eq!(down["targets"], json!([up["body_name"].clone()]));
        assert_ne!(up["sketch"], down["sketch"]);
        let t = out["twist_each_half_deg"].as_f64().unwrap();
        assert_eq!(up["twist"], json!(format!("{t:.6} deg")));
        assert_eq!(down["twist"], json!(format!("{:.6} deg", -t)));
    }

    #[test]
    fn bevel_sweeps_towards_the_apex_and_bores_only_itself() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Bevel, &json!({"teeth": 20, "mate_teeth": 40, "bore": 8})).unwrap();
        assert_eq!(
            names(&h),
            [
                "sketch.create", "sketch.line", "sketch.finish",
                "sketch.create", "sketch.line", "sketch.finish",
                "solid.sweep",
                "sketch.create", "sketch.circle.center", "sketch.finish", "solid.extrude",
            ]
        );
        assert_eq!(h.0[6].1["scale"], json!("0.666667"));
        assert!(h.0[6].1.get("twist").is_none());
        let cut = &h.0[10].1;
        assert_eq!((cut["operation"].clone(), cut["targets"].clone()), (json!("cut"), json!([out["body"].clone()])));
        assert!((out["pitch_angle_deg"].as_f64().unwrap() - 0.5f64.atan().to_degrees()).abs() < 1e-9);
        // The mate is the other half of the 90-degree pair.
        let mut h2 = Fake::default();
        let mate = build(&mut h2, Kind::Bevel, &json!({"teeth": 40, "mate_teeth": 20})).unwrap();
        let sum = out["pitch_angle_deg"].as_f64().unwrap() + mate["pitch_angle_deg"].as_f64().unwrap();
        assert!((sum - 90.0).abs() < 1e-9);
        assert!((out["cone_distance_mm"].as_f64().unwrap() - mate["cone_distance_mm"].as_f64().unwrap()).abs() < 1e-9);
    }

    #[test]
    fn long_worms_are_joined_sections_that_line_up() {
        let mut h = Fake::default();
        // 10 turns of a single-start m2 worm: three sections of 3.33 turns.
        let lead = PI * 2.0;
        let out = build(&mut h, Kind::Worm, &json!({"length": 10.0 * lead})).unwrap();
        assert_eq!(out["sections"], json!(3));
        let sweeps: Vec<&Value> = h.0.iter().filter(|(c, _)| c == "solid.sweep").map(|(_, a)| a).collect();
        assert_eq!(sweeps.len(), 3);
        assert_eq!(sweeps[0]["operation"], json!("new"));
        assert!(sweeps[1..].iter().all(|s| s["operation"] == json!("join")));
        let twist: f64 = sweeps[0]["twist"].as_str().unwrap().trim_end_matches(" deg").parse().unwrap();
        assert!((twist - 1200.0).abs() < 1e-6);
        // Each section's sketch starts where the previous sweep ended, turned by its twist.
        let creates: Vec<&Value> = h.0.iter().filter(|(c, a)| c == "sketch.create" && a["plane"] == json!("XY")).map(|(_, a)| a).collect();
        assert_eq!(creates.len(), 3);
        assert!((creates[2]["offset"].as_f64().unwrap() - 2.0 * 10.0 * lead / 3.0).abs() < 1e-5);
        let lines: Vec<&Value> = h.0.iter().filter(|(c, a)| c == "sketch.line" && a["closed"] == json!(true)).map(|(_, a)| a).collect();
        let first_angle = |v: &Value| v["points"][0][1].as_f64().unwrap().atan2(v["points"][0][0].as_f64().unwrap());
        let turned = (first_angle(lines[1]) - first_angle(lines[0])).rem_euclid(2.0 * PI);
        assert!((turned - 1200f64.to_radians().rem_euclid(2.0 * PI)).abs() < 1e-5);

        let mut h = Fake::default();
        assert!(build(&mut h, Kind::Worm, &json!({"length": 41.0 * lead})).is_err());
        assert!(h.0.is_empty());
    }

    #[test]
    fn worm_wheel_is_a_helical_gear_at_the_worm_lead_angle() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::WormWheel, &json!({"teeth": 30, "starts": 2, "diameter_factor": 10})).unwrap();
        assert_eq!(names(&h).last(), Some(&"solid.sweep"));
        assert!((out["helix_angle_deg"].as_f64().unwrap() - 0.2f64.atan().to_degrees()).abs() < 1e-9);
        assert!((out["pitch_diameter_mm"].as_f64().unwrap() - 60.0).abs() < 1e-9);
        assert!((out["centre_distance_mm"].as_f64().unwrap() - 40.0).abs() < 1e-9);
        assert_eq!(out["ratio"], json!(15.0));
    }

    #[test]
    fn rack_builds_the_bar_and_a_meshing_pinion() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Rack, &json!({"teeth": 12, "pinion_teeth": 18, "pinion_bore": 6, "thickness": 8})).unwrap();
        assert_eq!(
            names(&h),
            [
                "sketch.create", "sketch.line", "sketch.finish", "solid.extrude",
                "sketch.create", "sketch.line", "sketch.circle.center", "sketch.finish", "solid.extrude",
            ]
        );
        assert_eq!(h.0[3].1["body_name"], out["body"]);
        assert_eq!(h.0[8].1["body_name"], out["pinion"]["body"]);
        assert_eq!(h.0[8].1["operation"], json!("new"));
        // The pinion sits over rack tooth 6, its centre one pitch radius above the pitch line.
        let centre = [6.5 * 2.0 * PI, 18.0];
        assert_eq!(h.0[6].1["center"], json!([round(centre[0]), round(centre[1])]));
        assert!((out["pinion"]["rack_travel_per_turn_mm"].as_f64().unwrap() - 36.0 * PI).abs() < 1e-9);

        let mut h = Fake::default();
        let out = build(&mut h, Kind::Rack, &json!({"pinion_teeth": 0})).unwrap();
        assert_eq!(names(&h), ["sketch.create", "sketch.line", "sketch.finish", "solid.extrude"]);
        assert!(out.get("pinion").is_none());
    }

    #[test]
    fn rack_refuses_bad_input_before_any_command() {
        for p in [json!({"teeth": 0}), json!({"pinion_teeth": 3}), json!({"pinion_bore": 100}), json!({"back": -1})] {
            let mut h = Fake::default();
            assert!(build(&mut h, Kind::Rack, &p).is_err(), "{p}");
            assert!(h.0.is_empty(), "{p}");
        }
    }

    #[test]
    fn hypoid_members_are_twisted_cone_sweeps_of_opposite_hand() {
        let mut hg = Fake::default();
        let g = build(&mut hg, Kind::Hypoid, &json!({"member": "gear", "bore": 10})).unwrap();
        let mut hp = Fake::default();
        let p = build(&mut hp, Kind::Hypoid, &json!({"member": "pinion"})).unwrap();
        for h in [&hg, &hp] {
            let sweep = h.0.iter().find(|(c, _)| c == "solid.sweep").map(|(_, a)| a).unwrap();
            assert!(sweep.get("scale").is_some() && sweep.get("twist").is_some());
        }
        assert_eq!((g["hand"].clone(), p["hand"].clone()), (json!("right"), json!("left")));
        assert!(g["twist_deg"].as_f64().unwrap() > 0.0 && p["twist_deg"].as_f64().unwrap() < 0.0);
        assert_eq!(g["pinion"], p["pinion"]);
        assert_eq!(names(&hg).last(), Some(&"solid.extrude"), "bore cut");
        assert!(p["pinion"]["spiral_angle_deg"].as_f64().unwrap() > p["gear"]["spiral_angle_deg"].as_f64().unwrap());

        let mut h = Fake::default();
        assert!(build(&mut h, Kind::Hypoid, &json!({"member": "both"})).is_err());
        assert!(build(&mut h, Kind::Hypoid, &json!({"offset": 50})).is_err());
        assert!(h.0.is_empty());
    }

    #[test]
    fn internal_is_a_ring_sketch_extruded() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Internal, &json!({"teeth": 60, "pinion_teeth": 50})).unwrap();
        assert_eq!(names(&h), ["sketch.create", "sketch.line", "sketch.circle.center", "sketch.finish", "solid.extrude"]);
        assert_eq!(h.0[2].1["diameter"], json!(2.0 * (62.5 + 6.0)));
        assert_eq!(out["inner_diameter_mm"], json!(116.0));
        assert_eq!(out["warnings"].as_array().unwrap().len(), 1, "50 in 60 is too close");
        let mut h = Fake::default();
        assert!(build(&mut h, Kind::Internal, &json!({"teeth": 6})).is_err());
        assert!(h.0.is_empty());
    }

    #[test]
    fn planetary_builds_sun_planets_and_ring_as_separate_bodies() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Planetary, &json!({"sun_teeth": 12, "planet_teeth": 18, "planets": 3, "planet_bore": 5})).unwrap();
        let extrudes: Vec<&Value> = h.0.iter().filter(|(c, _)| c == "solid.extrude").map(|(_, a)| a).collect();
        assert_eq!(extrudes.len(), 5);
        assert!(extrudes.iter().all(|e| e["operation"] == json!("new")));
        assert_eq!(out["ring_teeth"], json!(48));
        assert_eq!(out["ratio_ring_fixed"], json!(5.0));
        // Each planet's bore sits at its centre.
        let bores: Vec<&Value> = h.0.iter().filter(|(c, _)| c == "sketch.circle.center").map(|(_, a)| a).collect();
        assert_eq!(bores.len(), 4, "three planet bores and the ring's outer circle");
        assert_eq!(bores[0]["center"], json!([30.0, 0.0]));
        assert_eq!(out["planet_centres_mm"][0], json!([30.0, 0.0]));

        let mut h = Fake::default();
        assert!(build(&mut h, Kind::Planetary, &json!({"planets": 7})).is_err());
        assert!(h.0.is_empty());
    }

    #[test]
    fn cycloidal_builds_a_placed_pair() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Cycloidal, &json!({"teeth": 60, "mate_teeth": 10, "module": 1, "mate_bore": 2})).unwrap();
        assert_eq!(out["bodies"].as_array().unwrap().len(), 2);
        assert_eq!(out["centre_distance_mm"], json!(35.0));
        let mate_bore = h.0.iter().find(|(c, _)| c == "sketch.circle.center").map(|(_, a)| a).unwrap();
        assert_eq!(mate_bore["center"], json!([35.0, 0.0]));
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Cycloidal, &json!({"build": "gear"})).unwrap();
        assert_eq!(out["bodies"].as_array().unwrap().len(), 1);
        // Equal gears get distinct body names.
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Cycloidal, &json!({"teeth": 20, "mate_teeth": 20})).unwrap();
        assert_ne!(out["bodies"][0], out["bodies"][1]);
    }

    #[test]
    fn cycloidal_drive_builds_disc_with_holes_and_housing() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::CycloidalDrive, &json!({"disc_bore": 10, "output_holes": 6, "output_pin_diameter": 4, "output_circle_diameter": 34})).unwrap();
        assert_eq!(out["ratio"], json!(11.0));
        let holes: Vec<&Value> = h.0.iter().filter(|(c, a)| c == "sketch.circle.center" && a["diameter"] == json!(7.0)).map(|(_, a)| a).collect();
        assert_eq!(holes.len(), 6);
        assert_eq!(holes[0]["center"], json!([18.5, 0.0]), "on the disc's centre, which is offset by e");
        let extrudes = h.0.iter().filter(|(c, _)| c == "solid.extrude").count();
        assert_eq!(extrudes, 2);
        // Holes that cannot fit are refused before anything is built.
        let mut h = Fake::default();
        assert!(build(&mut h, Kind::CycloidalDrive, &json!({"output_holes": 20, "output_circle_diameter": 34})).is_err());
        assert!(build(&mut h, Kind::CycloidalDrive, &json!({"eccentricity": 10})).is_err());
        assert!(h.0.is_empty());
    }

    #[test]
    fn crowned_spur_is_a_loft_of_thinner_end_sections() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Spur, &json!({"teeth": 20, "thickness": 12, "crowning": 0.05, "root_fillet": 0.38})).unwrap();
        let loft = h.0.iter().find(|(c, _)| c == "solid.loft").map(|(_, a)| a).unwrap();
        assert_eq!(loft["sections"].as_array().unwrap().len(), 7);
        assert!(h.0.iter().all(|(c, _)| c != "solid.extrude"));
        // Sections sit at even heights through the face width.
        let offsets: Vec<f64> = h.0.iter().filter(|(c, _)| c == "sketch.create").map(|(_, a)| a["offset"].as_f64().unwrap_or(0.0)).collect();
        assert_eq!(offsets, [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0]);
        // The end sections' teeth are thinner than the middle's: compare a tooth flank point.
        let lines: Vec<&Value> = h.0.iter().filter(|(c, _)| c == "sketch.line").map(|(_, a)| a).collect();
        let angle = |v: &Value, i: usize| v["points"][i][1].as_f64().unwrap().atan2(v["points"][i][0].as_f64().unwrap());
        let i = 10;
        // Tooth 0's first flank lies at negative angles; a thinner tooth brings it closer to 0.
        assert!(angle(lines[0], i) < 0.0);
        assert!(angle(lines[0], i) > angle(lines[3], i) + 1e-4, "end tooth is thinner than the middle");
        assert!((angle(lines[0], i) - angle(lines[6], i)).abs() < 1e-9, "both ends match");
        assert_eq!(out["crowning_mm"], json!(0.05));
        assert_eq!(out["root_fillet_mm"], json!(0.76));
        // Too much crowning is refused, by name, before anything is built.
        let mut h = Fake::default();
        assert!(matches!(build(&mut h, Kind::Spur, &json!({"crowning": 1.0})), Err(Failure::Input("crowning", _))));
        assert!(h.0.is_empty());
    }

    #[test]
    fn shift_and_undercut_advice_reach_the_reply() {
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Spur, &json!({"teeth": 10})).unwrap();
        let w = out["warnings"][0].as_str().unwrap();
        assert!(w.contains("profile shift of 0.415"), "{w}");
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Spur, &json!({"teeth": 10, "profile_shift": 0.45})).unwrap();
        assert!(out["warnings"].as_array().unwrap().is_empty());
        assert!(out["centre_distance_note"].as_str().unwrap().contains("x2 = -x1"));
        assert_eq!(out["tip_diameter_mm"], json!(2.0 * (10.0 + 2.0 * 1.45)));
        // Helical takes the shift in the normal plane.
        let mut h = Fake::default();
        assert!(build(&mut h, Kind::Helical, &json!({"profile_shift": 0.3, "tip_relief": 0.04})).is_ok());
        // A planetary set refuses a shift but takes a fillet.
        let mut h = Fake::default();
        assert!(build(&mut h, Kind::Planetary, &json!({"root_fillet": 0.38})).is_ok());
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Planetary, &json!({"profile_shift": 0.3, "root_fillet": 0.38})).unwrap();
        assert_eq!(out["ring_teeth"], json!(48), "shift is ignored for planetary sets");
        // The rack's pinion moves out with its shift.
        let mut h = Fake::default();
        let out = build(&mut h, Kind::Rack, &json!({"pinion_teeth": 10, "profile_shift": 0.5})).unwrap();
        assert!((out["pinion"]["centre_in_sketch_mm"][1].as_f64().unwrap() - 11.0).abs() < 1e-9);
    }
}

