//! Writes an SVG of the profiles the plugin draws: `cargo run --example svg > preview.svg`
//! Top: a 20-tooth gear meshing with a 12-tooth pinion. Bottom: a bevel gear's heel profile
//! (20:40) and the sections of 1- and 3-start worms, square to their axes.
use gears::gear::{BevelGear, SpurGear, Worm};
use std::f64::consts::PI;

fn path(pts: &[[f64; 2]], dx: f64, dy: f64, rot: f64) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let (x, y) = (p[0] * rot.cos() - p[1] * rot.sin() + dx, p[0] * rot.sin() + p[1] * rot.cos() + dy);
        d += &format!("{}{:.4},{:.4} ", if i == 0 { "M" } else { "L" }, x, -y);
    }
    d + "Z"
}

fn main() {
    let a = SpurGear { teeth: 20, module: 2.0, pressure_angle: 20.0, helix_angle: 0.0, backlash: 0.0, flank_points: 8 };
    let b = SpurGear { teeth: 12, ..a.clone() };
    let cd = f64::from(a.teeth + b.teeth) * a.module / 2.0;
    let bevel = BevelGear { teeth: 20, mate_teeth: 40, module: 2.0, pressure_angle: 20.0, backlash: 0.0, flank_points: 8, face_width: 0.0 };
    let worm = |starts| Worm { starts, module: 2.0, diameter_factor: 10.0, pressure_angle: 20.0, backlash: 0.0, flank_points: 8 };
    let shapes = [
        (a.profile().expect("gear"), 0.0, 0.0, 0.0, "#4a7bd0"),
        // Pinion turned half a tooth so its tooth sits in the gear's gap.
        (b.profile().expect("pinion"), cd, 0.0, PI + PI / f64::from(b.teeth), "#d07a4a"),
        (bevel.profile().expect("bevel"), 0.0, -52.0, 0.0, "#4aa070"),
        (worm(1).section(0.0).expect("worm"), 34.0, -52.0, 0.0, "#9a5ac8"),
        (worm(3).section(0.0).expect("worm"), 62.0, -52.0, 0.0, "#9a5ac8"),
    ];
    println!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-26 -26 104 104" width="1040" height="1040"><rect x="-26" y="-26" width="104" height="104" fill="#fff"/>"##);
    for (pts, dx, dy, rot, c) in &shapes {
        println!(r#"<path d="{}" fill="{c}" fill-opacity="0.35" stroke="{c}" stroke-width="0.08"/>"#, path(pts, *dx, *dy, *rot));
    }
    for (r, x) in [(a.circles().pitch, 0.0), (b.circles().pitch, cd)] {
        println!(r##"<circle cx="{x}" cy="0" r="{r}" fill="none" stroke="#444" stroke-width="0.05" stroke-dasharray="0.5 0.5"/>"##);
    }
    let label = |x: f64, y: f64, t: &str| println!(r##"<text x="{x}" y="{y}" font-family="sans-serif" font-size="2.2" text-anchor="middle" fill="#333">{t}</text>"##);
    label(0.0, 25.0, "spur 20 + 12, meshing");
    label(0.0, 77.0, "bevel 20:40, heel");
    label(34.0, 77.0, "worm, 1 start");
    label(62.0, 77.0, "worm, 3 starts");
    println!("</svg>");
}
