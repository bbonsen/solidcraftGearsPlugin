//! Writes a toolbar icon for each of the plugin's commands into `icons/`, drawn from the same
//! geometry the commands build: `cargo run --example icons`
//!
//! The icons follow SolidCraft's icon conventions where a plugin can: a 64 × 64 view box with no
//! intrinsic size, artwork centred on (32, 32) inside 5…59, 2.55-unit outlines with round caps
//! and joins, and a single colour (`currentColor`) so the host can tint them. Gear outlines use
//! few teeth so they stay legible at toolbar size.
use gears::gear::{CycloidalDrive, CycloidalGear, InternalGear, Mods, Planetary, Rack, SpurGear};
use std::f64::consts::PI;
use std::fmt::Write as _;

/// One drawn part: filled (outer outline and holes, even-odd) or a stroked line.
enum Part {
    Fill(Vec<Vec<[f64; 2]>>),
    Line(Vec<[f64; 2]>, bool),
}

const STROKE: f64 = 2.55;

fn spur(teeth: u32, module: f64) -> SpurGear {
    SpurGear { teeth, module, pressure_angle: 20.0, helix_angle: 0.0, backlash: 0.0, flank_points: 10, mods: Mods::default() }
}

fn circle(c: [f64; 2], r: f64) -> Vec<[f64; 2]> {
    (0..96).map(|i| {
        let a = 2.0 * PI * f64::from(i) / 96.0;
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    }).collect()
}

/// A part drawn slightly smaller about its own centre, so a visible gap separates it from the
/// part it meshes with at toolbar size (an icon simplification; the tooth shapes are unchanged).
fn inset(pts: &[[f64; 2]], c: [f64; 2], k: f64) -> Vec<[f64; 2]> {
    pts.iter().map(|p| [c[0] + (p[0] - c[0]) * k, c[1] + (p[1] - c[1]) * k]).collect()
}

fn moved(pts: &[[f64; 2]], d: [f64; 2], rot: f64) -> Vec<[f64; 2]> {
    let (s, c) = rot.sin_cos();
    pts.iter().map(|p| [p[0] * c - p[1] * s + d[0], p[0] * s + p[1] * c + d[1]]).collect()
}

/// Side view of a cylinder-like gear (axis vertical, y up): the silhouette and the visible tooth
/// crest lines, whose turn at height t ∈ [0, 1] is `turn(t)` and radius `radius(t)`.
fn side_view(teeth: u32, height: f64, turn: impl Fn(f64) -> f64, radius: impl Fn(f64) -> f64) -> Vec<Part> {
    let mut parts = Vec::new();
    let steps = 48;
    let edge = |sign: f64| -> Vec<[f64; 2]> {
        (0..=steps).map(|i| {
            let t = f64::from(i) / f64::from(steps);
            [sign * radius(t), height * t]
        }).collect()
    };
    let mut outline = edge(1.0);
    outline.extend(edge(-1.0).into_iter().rev());
    parts.push(Part::Line(outline, true));
    for k in 0..teeth {
        let phi = 2.0 * PI * (f64::from(k) + 0.5) / f64::from(teeth);
        let mut run: Vec<[f64; 2]> = Vec::new();
        for i in 0..=steps {
            let t = f64::from(i) / f64::from(steps);
            let th = phi + turn(t);
            // Only the front half of the gear faces the viewer; keep clear of the silhouette.
            if th.cos() > 0.25 {
                run.push([radius(t) * th.sin(), height * t]);
            } else if run.len() > 1 {
                parts.push(Part::Line(std::mem::take(&mut run), false));
            } else {
                run.clear();
            }
        }
        if run.len() > 1 {
            parts.push(Part::Line(run, false));
        }
    }
    parts
}

/// A worm's axial section (side view, axis horizontal): crests along the top, and along the
/// bottom half a lead further on, joined at the ends.
fn worm_outline(module: f64, pitch_r: f64, turns: f64) -> Vec<[f64; 2]> {
    let p = PI * module;
    let tan_a = 20f64.to_radians().tan();
    let (tip, root) = (pitch_r + module, pitch_r - 1.25 * module);
    let len = turns * p;
    // One edge: crests of half-thickness s(y) about centres x_c, along x ∈ [0, len].
    let edge = |shift: f64| -> Vec<[f64; 2]> {
        let (s_tip, s_root) = (p / 4.0 - module * tan_a, p / 4.0 + 1.25 * module * tan_a);
        let mut out = vec![[0.0, root]];
        let mut k = -1.0;
        while (k + 0.5) * p + shift < len + p {
            let xc = (k + 0.5) * p + shift;
            for [x, y] in [[xc - s_root, root], [xc - s_tip, tip], [xc + s_tip, tip], [xc + s_root, root]] {
                if (0.0..=len).contains(&x) {
                    out.push([x, y]);
                }
            }
            k += 1.0;
        }
        out.push([len, root]);
        out
    };
    let top = edge(0.0);
    let bottom: Vec<[f64; 2]> = edge(p / 2.0).into_iter().map(|[x, y]| [x, -y]).rev().collect();
    let mut outline = top;
    outline.push([len, -root]);
    outline.extend(bottom);
    outline.push([0.0, root]);
    outline
}

/// The visible thread crests across the front of a worm.
fn worm_crests(module: f64, pitch_r: f64, turns: f64) -> Vec<Part> {
    let p = PI * module;
    let mut parts = Vec::new();
    let mut k = 0.0;
    while (k + 0.5) * p < turns * p {
        let xc = (k + 0.5) * p;
        let run: Vec<[f64; 2]> = (0..=24)
            .map(|i| {
                let th = PI / 2.0 - PI * f64::from(i) / 24.0;
                [xc + p * (PI / 2.0 - th) / (2.0 * PI), pitch_r * 0.82 * th.sin()]
            })
            .filter(|q| q[0] > 0.3 && q[0] < turns * p - 0.3)
            .collect();
        if run.len() > 1 {
            parts.push(Part::Line(run, false));
        }
        k += 1.0;
    }
    parts
}

fn icons() -> Vec<(&'static str, Vec<Part>)> {
    let mut out = Vec::new();

    // Spur: a 12-tooth gear with a bore.
    let g = spur(12, 1.0);
    out.push(("spur", vec![Part::Fill(vec![g.profile().expect("spur"), circle([0.0, 0.0], 1.6)])]));

    // Helical: side view, crest lines turning steadily with height.
    out.push(("helical", side_view(9, 1.5, |t| 0.75 * t, |_| 1.0)));

    // Herringbone: side view, crest lines turning out and back.
    out.push(("herringbone", side_view(9, 1.5, |t| 0.75 * (1.0 - (2.0 * t - 1.0).abs()), |_| 1.0)));

    // Internal: a ring with 18 inward teeth.
    let ring = InternalGear { teeth: 18, module: 1.0, pressure_angle: 20.0, backlash: 0.0, flank_points: 8, rim: 1.4 };
    out.push(("internal", vec![Part::Fill(vec![circle([0.0, 0.0], ring.outer_radius()), ring.profile(0.0).expect("ring")])]));

    // Planetary: ring filled, sun and three planets outlined, in mesh.
    let p = Planetary {
        sun_teeth: 9,
        planet_teeth: 9,
        planets: 3,
        module: 1.0,
        pressure_angle: 20.0,
        backlash: 0.1,
        flank_points: 8,
        rim: 1.3,
        mods: Mods::default(),
    };
    let ((sun, _), planets, (ring_pts, _)) = p.layout().expect("planetary");
    let mut parts = vec![Part::Fill(vec![circle([0.0, 0.0], p.ring().outer_radius()), ring_pts])];
    parts.push(Part::Fill(vec![inset(&sun, [0.0, 0.0], 0.86)]));
    parts.extend(planets.into_iter().map(|(pts, c)| Part::Fill(vec![inset(&pts, c, 0.86)])));
    out.push(("planetary", parts));

    // Rack and pinion: the rack filled, a 10-tooth pinion outlined in mesh above it.
    let r = Rack { teeth: 5, module: 1.0, pressure_angle: 20.0, backlash: 0.1, back: 1.3 };
    let pinion = r.pinion(10, 8);
    let (pp, c) = r.pinion_in_mesh(&pinion, 2).expect("pinion");
    let lifted: Vec<[f64; 2]> = inset(&pp, c, 0.88).iter().map(|q| [q[0], q[1] + 0.35]).collect();
    out.push(("rack", vec![Part::Fill(vec![r.profile().expect("rack")]), Part::Fill(vec![lifted, circle([c[0], c[1] + 0.35], 1.2)])]));

    // Bevel: side view of the cone frustum, crest lines running to the apex.
    out.push(("bevel", side_view(12, 1.2, |_| 0.0, |t| 1.3 - 0.75 * t)));

    // Hypoid: the same cone with curved (spiral) crest lines.
    out.push(("hypoid", side_view(12, 1.2, |t| 0.8 * t * (1.0 + 0.4 * t), |t| 1.3 - 0.75 * t)));

    // Worm: side view, axial section with the crests crossing the front.
    let mut parts = vec![Part::Line(worm_outline(1.0, 3.0, 4.0), true)];
    parts.extend(worm_crests(1.0, 3.0, 4.0));
    out.push(("worm", parts));

    // Worm wheel: the wheel filled, its worm crossing above it, outlined.
    let wheel = spur(16, 1.0);
    let rw = wheel.circles().pitch;
    let worm_r = 2.0;
    let len = 3.0 * PI;
    let worm: Vec<[f64; 2]> = worm_outline(1.0, worm_r, 3.0).iter().map(|q| [q[0] - len / 2.0, q[1] + rw + worm_r + 0.9]).collect();
    out.push(("worm-wheel", vec![Part::Fill(vec![inset(&wheel.profile().expect("wheel"), [0.0, 0.0], 0.94), circle([0.0, 0.0], 1.8)]), Part::Fill(vec![worm])]));

    // Cycloidal: a 16-tooth gear filled, its 8-tooth mate outlined in mesh.
    let cg = CycloidalGear { teeth: 16, mate_teeth: 8, module: 1.0, backlash: 0.1, flank_points: 10 };
    let ((a, _), (b, c)) = cg.pair().expect("cycloidal");
    out.push(("cycloidal", vec![Part::Fill(vec![a, circle([0.0, 0.0], 1.6)]), Part::Fill(vec![inset(&b, c, 0.84), circle(c, 0.9)])]));

    // Cycloidal drive: the pin housing filled, the lobed disc outlined on its eccentric.
    let d = CycloidalDrive { pins: 8, pin_circle: 9.0, pin_radius: 1.3, eccentricity: 0.7, lobe_points: 30 };
    let disc = moved(&d.disc().expect("disc"), [d.eccentricity, 0.0], 0.0);
    out.push((
        "cycloidal-drive",
        vec![
            Part::Fill(vec![circle([0.0, 0.0], d.pin_circle + 1.6), d.housing_bore().expect("bore")]),
            Part::Line(disc, true),
            Part::Line(circle([d.eccentricity, 0.0], 1.5), true),
        ],
    ));
    out
}

fn path(pts: &[[f64; 2]], f: &impl Fn([f64; 2]) -> [f64; 2], closed: bool) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let q = f(*p);
        let _ = write!(d, "{}{:.2} {:.2}", if i == 0 { "M" } else { " L" }, q[0], q[1]);
    }
    if closed {
        d.push_str(" Z");
    }
    d
}

fn svg(parts: &[Part]) -> String {
    // Fit every point (and half a stroke around lines) into 5…59, centred on 32, y up.
    let all: Vec<[f64; 2]> = parts
        .iter()
        .flat_map(|p| match p {
            Part::Fill(rings) => rings.iter().flatten().copied().collect::<Vec<_>>(),
            Part::Line(pts, _) => pts.clone(),
        })
        .collect();
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in &all {
        for i in 0..2 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    let scale = (54.0 - STROKE) / span;
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let f = |p: [f64; 2]| [32.0 + (p[0] - mid[0]) * scale, 32.0 - (p[1] - mid[1]) * scale];
    let mut s = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n");
    for part in parts {
        match part {
            Part::Fill(rings) => {
                let d: Vec<String> = rings.iter().map(|r| path(r, &f, true)).collect();
                let _ = writeln!(s, "  <path d=\"{}\" fill=\"currentColor\" fill-rule=\"evenodd\"/>", d.join(" "));
            }
            Part::Line(pts, closed) => {
                let _ = writeln!(s, "  <path d=\"{}\" stroke=\"currentColor\" stroke-width=\"{STROKE}\"/>", path(pts, &f, *closed));
            }
        }
    }
    s.push_str("</svg>\n");
    s
}

fn main() -> std::io::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons");
    std::fs::create_dir_all(&dir)?;
    for (name, parts) in icons() {
        std::fs::write(dir.join(format!("{name}.svg")), svg(&parts))?;
        println!("icons/{name}.svg");
    }
    Ok(())
}
