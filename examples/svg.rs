//! Writes an SVG of two meshing gear profiles: `cargo run --example svg > gears.svg`
use gears::gear::SpurGear;
fn path(g: &SpurGear, dx: f64, rot: f64) -> String {
    let pts = g.profile().expect("valid gear");
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let (x, y) = (p[0] * rot.cos() - p[1] * rot.sin() + dx, p[0] * rot.sin() + p[1] * rot.cos());
        d += &format!("{}{:.4},{:.4} ", if i == 0 { "M" } else { "L" }, x, -y);
    }
    d + "Z"
}
fn main() {
    let a = SpurGear { teeth: 20, module: 2.0, pressure_angle: 20.0, helix_angle: 0.0, backlash: 0.0, flank_points: 8 };
    let b = SpurGear { teeth: 12, ..a.clone() };
    let cd = (a.teeth + b.teeth) as f64 * a.module / 2.0;
    // Pinion rotated half a tooth so its tooth sits in the gear's gap.
    let rot = std::f64::consts::PI + std::f64::consts::PI / b.teeth as f64;
    println!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-26 -26 72 52" width="1080" height="780"><rect x="-26" y="-26" width="72" height="52" fill="#fff"/>"##);
    for (d, c) in [(path(&a, 0.0, 0.0), "#4a7bd0"), (path(&b, cd, rot), "#d07a4a")] {
        println!(r#"<path d="{d}" fill="{c}" fill-opacity="0.35" stroke="{c}" stroke-width="0.08"/>"#);
    }
    for (r, x) in [(a.circles().pitch, 0.0), (b.circles().pitch, cd)] {
        println!(r##"<circle cx="{x}" cy="0" r="{r}" fill="none" stroke="#444" stroke-width="0.05" stroke-dasharray="0.5 0.5"/>"##);
    }
    println!("</svg>");
}
