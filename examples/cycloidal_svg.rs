//! Writes an SVG of a cycloidal-tooth pair (60:10, in mesh) and a 12-pin cycloidal drive:
//! `cargo run --example cycloidal_svg > cycloidal.svg`
use gears::gear::{CycloidalDrive, CycloidalGear};

fn path(pts: &[[f64; 2]], dx: f64, dy: f64) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        d += &format!("{}{:.4},{:.4} ", if i == 0 { "M" } else { "L" }, p[0] + dx, -(p[1] + dy));
    }
    d + "Z"
}

fn main() {
    let g = CycloidalGear { teeth: 60, mate_teeth: 10, module: 1.0, backlash: 0.05, flank_points: 12 };
    let ((a, _), (b, _)) = g.pair().expect("pair");
    let d = CycloidalDrive { pins: 12, pin_circle: 30.0, pin_radius: 3.0, eccentricity: 1.5, lobe_points: 60 };
    let disc: Vec<[f64; 2]> = d.disc().expect("disc").iter().map(|p| [p[0] + d.eccentricity, p[1]]).collect();
    let bore = d.housing_bore().expect("bore");
    let (dx, dy, r) = (0.0, -75.0, d.pin_circle + 5.0);
    println!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-34 -34 82 152" width="660" height="1224"><rect x="-34" y="-34" width="82" height="152" fill="#fff"/>"##);
    println!(r##"<path d="{}" fill="#4a7bd0" fill-opacity="0.35" stroke="#2a5bb0" stroke-width="0.08"/>"##, path(&a, 0.0, 0.0));
    println!(r##"<path d="{}" fill="#d07a4a" fill-opacity="0.45" stroke="#b05a2a" stroke-width="0.08"/>"##, path(&b, 0.0, 0.0));
    println!(r##"<path d="M{},{} a{r},{r} 0 1,0 {},0 a{r},{r} 0 1,0 {},0 Z {}" fill="#888" fill-opacity="0.35" fill-rule="evenodd" stroke="#666" stroke-width="0.1"/>"##, dx + r, -dy, -2.0 * r, 2.0 * r, path(&bore, dx, dy));
    println!(r##"<path d="{}" fill="#4aa070" fill-opacity="0.45" stroke="#2a8050" stroke-width="0.1"/>"##, path(&disc, dx, dy));
    println!(r##"<text x="7" y="36" font-family="sans-serif" font-size="3" text-anchor="middle" fill="#333">cycloidal teeth, 60:10 in mesh</text>"##);
    println!(r##"<text x="0" y="115" font-family="sans-serif" font-size="3" text-anchor="middle" fill="#333">cycloidal drive, 12 pins, 11 lobes</text>"##);
    println!("</svg>");
}
