//! Writes an SVG comparing tooth modifications on a 14-tooth, module-2 gear, two teeth each:
//! `cargo run --example mods_svg > mods.svg`
use gears::gear::{Mods, SpurGear};
use std::f64::consts::PI;

fn main() {
    let base = SpurGear { teeth: 14, module: 2.0, pressure_angle: 20.0, helix_angle: 0.0, backlash: 0.0, flank_points: 16, mods: Mods::default() };
    let cases = [
        ("standard", Mods::default()),
        ("profile shift x = 0.5", Mods { shift: 0.5, ..Mods::default() }),
        ("root fillet 0.38 m", Mods { root_fillet: 0.38, ..Mods::default() }),
        ("tip relief 0.2 mm", Mods { tip_relief: 0.2, ..Mods::default() }),
    ];
    println!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 34" width="1200" height="340"><rect width="120" height="34" fill="#fff"/>"##);
    for (i, (label, mods)) in cases.iter().enumerate() {
        let g = SpurGear { mods: mods.clone(), ..base.clone() };
        let pts = g.profile().expect("valid");
        let ox = 15.0 + 30.0 * i as f64;
        // Show the teeth around +y: turn the gear a quarter, drop it so the root sits low.
        let rot = PI / 2.0;
        let mut d = String::new();
        let mut first = true;
        for p in &pts {
            let (x, y) = (p[0] * rot.cos() - p[1] * rot.sin(), p[0] * rot.sin() + p[1] * rot.cos());
            if y < 10.0 || x.abs() > 7.0 { first = true; continue; }
            d += &format!("{}{:.4},{:.4} ", if first { "M" } else { "L" }, ox + 2.0 * x, 27.0 - (y - 10.0) * 2.0);
            first = false;
        }
        let refd = if *mods == Mods::default() { String::new() } else {
            let mut r = String::new();
            let mut first = true;
            for p in &base.profile().expect("valid") {
                let (x, y) = (p[0] * rot.cos() - p[1] * rot.sin(), p[0] * rot.sin() + p[1] * rot.cos());
                if y < 10.0 || x.abs() > 7.0 { first = true; continue; }
                r += &format!("{}{:.4},{:.4} ", if first { "M" } else { "L" }, ox + 2.0 * x, 27.0 - (y - 10.0) * 2.0);
                first = false;
            }
            r
        };
        if !refd.is_empty() {
            println!(r##"<path d="{refd}" fill="none" stroke="#999" stroke-width="0.15" stroke-dasharray="0.6 0.4"/>"##);
        }
        println!(r##"<path d="{d}" fill="none" stroke="#2a5bb0" stroke-width="0.22"/>"##);
        println!(r##"<text x="{ox}" y="32" font-family="sans-serif" font-size="2.2" text-anchor="middle" fill="#333">{label}</text>"##);
    }
    println!("</svg>");
}
