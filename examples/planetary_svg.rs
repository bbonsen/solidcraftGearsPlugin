//! Writes an SVG of a planetary set as the plugin lays it out:
//! `cargo run --example planetary_svg > planetary.svg`
use gears::gear::Planetary;

fn path(pts: &[[f64; 2]]) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        d += &format!("{}{:.4},{:.4} ", if i == 0 { "M" } else { "L" }, p[0], -p[1]);
    }
    d + "Z"
}

fn main() {
    let p = Planetary { sun_teeth: 12, planet_teeth: 18, planets: 3, module: 2.0, pressure_angle: 20.0, backlash: 0.05, flank_points: 8, rim: 6.0 };
    let ((sun, _), planets, (ring, _)) = p.layout().expect("valid set");
    let r = p.ring().outer_radius();
    let v = r + 2.0;
    println!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="900" height="900"><rect x="{0}" y="{1}" width="{2}" height="{3}" fill="#fff"/>"##, -v, -v, 2.0 * v, 2.0 * v);
    // Ring: outer circle with the toothed outline as a hole.
    println!(r##"<path d="M{r},0 A{r},{r} 0 1,0 {},0 A{r},{r} 0 1,0 {r},0 Z {}" fill="#888" fill-opacity="0.35" fill-rule="evenodd" stroke="#666" stroke-width="0.12"/>"##, -r, path(&ring));
    println!(r##"<path d="{}" fill="#d0a24a" fill-opacity="0.45" stroke="#b07a1a" stroke-width="0.12"/>"##, path(&sun));
    for (pts, _) in &planets {
        println!(r##"<path d="{}" fill="#4a7bd0" fill-opacity="0.35" stroke="#2a5bb0" stroke-width="0.12"/>"##, path(pts));
    }
    println!("</svg>");
}
