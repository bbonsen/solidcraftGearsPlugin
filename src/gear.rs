//! Involute spur gear geometry. Pure math, no host calls, so it is tested natively.

use std::f64::consts::PI;

/// Inputs of a standard (unshifted, full-depth) involute spur or helical gear. Lengths in mm.
/// For a helical gear, `module`, `pressure_angle` and `backlash` are normal-plane values (what
/// a hob or catalogue gives); the profile is drawn in the transverse plane.
#[derive(Clone, Debug)]
pub struct SpurGear {
    pub teeth: u32,
    /// Normal module.
    pub module: f64,
    /// Normal pressure angle in degrees.
    pub pressure_angle: f64,
    /// Helix angle in degrees; 0 for a spur gear.
    pub helix_angle: f64,
    /// Normal circular backlash at the pitch circle, removed from tooth thickness (mm).
    pub backlash: f64,
    /// Points along each flank (more = smoother, slower).
    pub flank_points: u32,
}

/// Key circles of the gear, all radii in mm.
#[derive(Clone, Debug, PartialEq)]
pub struct Circles {
    pub pitch: f64,
    pub base: f64,
    pub tip: f64,
    pub root: f64,
}

pub const MIN_TEETH: u32 = 6;
pub const MAX_TEETH: u32 = 150;

fn inv(alpha: f64) -> f64 {
    alpha.tan() - alpha
}

impl SpurGear {
    pub fn validate(&self) -> Result<(), (&'static str, String)> {
        if !(MIN_TEETH..=MAX_TEETH).contains(&self.teeth) {
            return Err(("teeth", format!("must be {MIN_TEETH}..={MAX_TEETH}")));
        }
        if !(self.module.is_finite() && self.module >= 0.1 && self.module <= 50.0) {
            return Err(("module", "must be 0.1..=50 mm".into()));
        }
        if !(self.pressure_angle.is_finite() && (10.0..=35.0).contains(&self.pressure_angle)) {
            return Err(("pressure_angle", "must be 10..=35 degrees".into()));
        }
        if !(self.backlash.is_finite() && self.backlash >= 0.0 && self.backlash < self.module) {
            return Err(("backlash", "must be at least 0 and smaller than the module".into()));
        }
        if !(self.helix_angle.is_finite() && (0.0..=45.0).contains(&self.helix_angle)) {
            return Err(("helix_angle", "must be 0..=45 degrees".into()));
        }
        if !(2..=40).contains(&self.flank_points) {
            return Err(("flank_points", "must be 2..=40".into()));
        }
        Ok(())
    }

    fn cos_helix(&self) -> f64 {
        self.helix_angle.to_radians().cos()
    }

    /// Module in the transverse plane (equals `module` for a spur gear).
    pub fn transverse_module(&self) -> f64 {
        self.module / self.cos_helix()
    }

    /// Pressure angle in the transverse plane, radians.
    pub fn transverse_pressure_angle(&self) -> f64 {
        (self.pressure_angle.to_radians().tan() / self.cos_helix()).atan()
    }

    /// How far the profile turns over `face_width` mm of a helical gear, radians.
    pub fn twist(&self, face_width: f64) -> f64 {
        face_width * self.helix_angle.to_radians().tan() / self.circles().pitch
    }

    pub fn circles(&self) -> Circles {
        let pitch = self.transverse_module() * f64::from(self.teeth) / 2.0;
        Circles {
            pitch,
            base: pitch * self.transverse_pressure_angle().cos(),
            tip: pitch + self.module,
            root: pitch - 1.25 * self.module,
        }
    }

    /// Fewest teeth that avoid undercut (2·cosβ / sin²αt; 2 / sin²α for a spur gear).
    pub fn min_teeth_without_undercut(&self) -> f64 {
        2.0 * self.cos_helix() / self.transverse_pressure_angle().sin().powi(2)
    }

    /// Closed profile, counter-clockwise, gear centred on the origin with tooth 0 on +X.
    /// The last point is not repeated; the caller closes the loop.
    pub fn profile(&self) -> Result<Vec<[f64; 2]>, (&'static str, String)> {
        self.validate()?;
        let c = self.circles();
        let backlash = self.backlash / self.cos_helix();
        let half_pitch = (PI * self.transverse_module() / 2.0 - backlash) / (2.0 * c.pitch);
        let flank = involute_flank(&c, self.transverse_pressure_angle(), half_pitch, self.flank_points as usize)?;
        assemble(self.teeth, &flank, 0.0)
    }
}

/// One side of a tooth from the root circle to the tip circle, as (radius, half angle):
/// the tooth spans `centre ± half angle` at that radius.
pub type Flank = Vec<(f64, f64)>;

/// An involute flank for `circles`, with transverse pressure angle `alpha` (radians) and half
/// the tooth's angular thickness at the pitch circle `half_pitch`. Below the base circle the
/// flank runs radially to the root.
pub fn involute_flank(c: &Circles, alpha: f64, half_pitch: f64, n: usize) -> Result<Flank, (&'static str, String)> {
    let psi_base = half_pitch + inv(alpha);
    let psi = |r: f64| psi_base - inv((c.base / r).clamp(-1.0, 1.0).acos());
    if psi(c.tip) <= 0.0 {
        return Err(("teeth", "teeth come to a point at the tip circle; use more teeth or a smaller pressure angle".into()));
    }
    let mut flank = Vec::with_capacity(n + 1);
    if c.root < c.base {
        flank.push((c.root, psi_base));
    }
    let start = c.root.max(c.base);
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        // Cluster points near the base where curvature is highest.
        let r = start + (c.tip - start) * t * t;
        flank.push((r, psi(r)));
    }
    Ok(flank)
}

/// Points on an arc land strictly between angles `a0` and `a1` at radius `r`: at least two, and
/// one per 4 degrees, so long lands (worm threads) stay round.
fn land(out: &mut Vec<[f64; 2]>, r: f64, a0: f64, a1: f64) {
    let n = (((a1 - a0).abs() / 4f64.to_radians()).ceil() as usize).clamp(2, 180);
    for j in 1..=n {
        let a = a0 + (a1 - a0) * j as f64 / (n + 1) as f64;
        out.push([r * a.cos(), r * a.sin()]);
    }
}

/// `teeth` copies of a symmetric tooth with `flank` (root to tip), the first centred at angle
/// `rotation`, joined by tip and root lands. Counter-clockwise; last point not repeated.
pub fn assemble(teeth: u32, flank: &[(f64, f64)], rotation: f64) -> Result<Vec<[f64; 2]>, (&'static str, String)> {
    let (Some(&(r_root, psi_root)), Some(&(r_tip, psi_tip))) = (flank.first(), flank.last()) else {
        return Err(("flank_points", "a flank needs points".into()));
    };
    let step = 2.0 * PI / f64::from(teeth);
    if psi_tip <= 0.0 {
        return Err(("teeth", "teeth come to a point at the tip circle".into()));
    }
    if 2.0 * psi_root >= step {
        return Err(("teeth", "no gap is left between teeth at the root circle".into()));
    }
    let pt = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
    let mut out = Vec::new();
    for k in 0..teeth {
        let th = rotation + step * f64::from(k);
        out.extend(flank.iter().map(|&(r, p)| pt(r, th - p)));
        land(&mut out, r_tip, th - psi_tip, th + psi_tip);
        out.extend(flank.iter().rev().map(|&(r, p)| pt(r, th + p)));
        land(&mut out, r_root, th + psi_root, th + step - psi_root);
    }
    Ok(out)
}

/// A straight bevel gear for shafts at 90 degrees, given at its large (heel) end. The teeth are
/// Tredgold's approximation: the heel profile is that of a spur gear with z / cos(pitch angle)
/// teeth on the back cone, laid into the plane square to the axis; from there every point runs
/// straight towards the cone apex.
#[derive(Clone, Debug)]
pub struct BevelGear {
    pub teeth: u32,
    pub mate_teeth: u32,
    /// Module at the heel.
    pub module: f64,
    pub pressure_angle: f64,
    pub backlash: f64,
    pub flank_points: u32,
    /// Face width along the pitch cone, mm; 0 picks a third of the cone distance.
    pub face_width: f64,
}

impl BevelGear {
    fn spur(&self, teeth: u32) -> SpurGear {
        SpurGear {
            teeth,
            module: self.module,
            pressure_angle: self.pressure_angle,
            helix_angle: 0.0,
            backlash: self.backlash,
            flank_points: self.flank_points,
        }
    }

    pub fn validate(&self) -> Result<(), (&'static str, String)> {
        self.spur(self.teeth).validate()?;
        if !(MIN_TEETH..=MAX_TEETH).contains(&self.mate_teeth) {
            return Err(("mate_teeth", format!("must be {MIN_TEETH}..={MAX_TEETH}")));
        }
        if !(self.face_width.is_finite() && self.face_width >= 0.0 && self.face_width <= self.cone_distance() / 2.0) {
            return Err(("face_width", format!("must be 0 (automatic) up to {:.3} mm, half the cone distance", self.cone_distance() / 2.0)));
        }
        Ok(())
    }

    /// Pitch cone angle δ, radians.
    pub fn pitch_angle(&self) -> f64 {
        f64::from(self.teeth).atan2(f64::from(self.mate_teeth))
    }

    pub fn pitch_radius(&self) -> f64 {
        self.module * f64::from(self.teeth) / 2.0
    }

    /// Distance from the heel's pitch circle to the apex along the pitch cone.
    pub fn cone_distance(&self) -> f64 {
        self.pitch_radius() / self.pitch_angle().sin()
    }

    /// Face width actually used.
    pub fn face(&self) -> f64 {
        if self.face_width > 0.0 {
            self.face_width
        } else {
            self.cone_distance() / 3.0
        }
    }

    /// Length of the teeth along the gear axis.
    pub fn axial_length(&self) -> f64 {
        self.face() * self.pitch_angle().cos()
    }

    /// Size of the toe (small end) relative to the heel.
    pub fn toe_scale(&self) -> f64 {
        1.0 - self.face() / self.cone_distance()
    }

    /// Axial distance from the heel plane to the cone apex.
    pub fn apex_distance(&self) -> f64 {
        self.pitch_radius() / self.pitch_angle().tan()
    }

    /// Heel profile in the plane square to the axis, tooth 0 on +X.
    pub fn profile(&self) -> Result<Vec<[f64; 2]>, (&'static str, String)> {
        self.validate()?;
        let delta = self.pitch_angle();
        let r = self.pitch_radius();
        let rv = r / delta.cos();
        let m = self.module;
        let virt = Circles { pitch: rv, base: rv * self.pressure_angle.to_radians().cos(), tip: rv + m, root: rv - 1.25 * m };
        let half_pitch = (PI * m / 2.0 - self.backlash) / (2.0 * rv);
        let vflank = involute_flank(&virt, self.pressure_angle.to_radians(), half_pitch, self.flank_points as usize)?;
        // Back cone → axis-square plane: depth from the pitch circle shrinks by cos δ, the arc
        // along the circle is kept.
        let flank: Flank = vflank
            .iter()
            .map(|&(rho, psi)| {
                let rr = r + (rho - rv) * delta.cos();
                (rr, rho * psi / rr)
            })
            .collect();
        assemble(self.teeth, &flank, 0.0)
    }
}

/// A cylindrical worm with a straight-sided axial profile (ZA / Archimedean). Its section square
/// to the axis is a gear outline with one tooth per start, so a twisted sweep of that section
/// (one turn per lead) is the exact thread.
#[derive(Clone, Debug)]
pub struct Worm {
    pub starts: u32,
    /// Axial module.
    pub module: f64,
    /// Diameter factor q: pitch diameter = q × module.
    pub diameter_factor: f64,
    /// Axial pressure angle in degrees.
    pub pressure_angle: f64,
    pub backlash: f64,
    pub flank_points: u32,
}

impl Worm {
    pub fn validate(&self) -> Result<(), (&'static str, String)> {
        if !(1..=6).contains(&self.starts) {
            return Err(("starts", "must be 1..=6".into()));
        }
        if !(self.module.is_finite() && self.module >= 0.1 && self.module <= 50.0) {
            return Err(("module", "must be 0.1..=50 mm".into()));
        }
        if !(self.diameter_factor.is_finite() && (5.0..=30.0).contains(&self.diameter_factor)) {
            return Err(("diameter_factor", "must be 5..=30".into()));
        }
        if !(self.pressure_angle.is_finite() && (10.0..=30.0).contains(&self.pressure_angle)) {
            return Err(("pressure_angle", "must be 10..=30 degrees".into()));
        }
        if !(self.backlash.is_finite() && self.backlash >= 0.0 && self.backlash < self.module / 2.0) {
            return Err(("backlash", "must be at least 0 and smaller than half the module".into()));
        }
        if !(2..=40).contains(&self.flank_points) {
            return Err(("flank_points", "must be 2..=40".into()));
        }
        Ok(())
    }

    pub fn circles(&self) -> Circles {
        let pitch = self.diameter_factor * self.module / 2.0;
        // No base circle: the axial flanks are straight. Report the pitch radius in its place.
        Circles { pitch, base: pitch, tip: pitch + self.module, root: pitch - 1.25 * self.module }
    }

    /// Axial distance the thread advances per turn.
    pub fn lead(&self) -> f64 {
        f64::from(self.starts) * PI * self.module
    }

    /// Lead angle γ at the pitch diameter, radians.
    pub fn lead_angle(&self) -> f64 {
        (f64::from(self.starts) / self.diameter_factor).atan()
    }

    /// Section square to the axis at the profile plane, thread 0 centred on +X, turned by
    /// `rotation` radians.
    pub fn section(&self, rotation: f64) -> Result<Vec<[f64; 2]>, (&'static str, String)> {
        self.validate()?;
        let c = self.circles();
        let tan_a = self.pressure_angle.to_radians().tan();
        // Axial tooth thickness at radius r; its angular half-width is π·t / lead.
        let t = |r: f64| PI * self.module / 2.0 - self.backlash - 2.0 * (r - c.pitch) * tan_a;
        if t(c.tip) <= 0.0 || PI * self.module - t(c.root) <= 0.0 {
            return Err(("pressure_angle", "the thread comes to a point; use a smaller pressure angle".into()));
        }
        let lead = self.lead();
        let psi = |r: f64| PI * t(r) / lead;
        // The flank is a spiral; keep a point every 3 degrees of it.
        let sweep = (psi(c.root) - psi(c.tip)).to_degrees();
        let n = (self.flank_points as usize).max((sweep / 3.0).ceil() as usize + 1).min(120);
        let flank: Flank = (0..n)
            .map(|i| {
                let r = c.root + (c.tip - c.root) * i as f64 / (n - 1) as f64;
                (r, psi(r))
            })
            .collect();
        assemble(self.starts, &flank, rotation)
    }

    /// The matching wheel, as the helical gear that meshes with this worm on crossed axes:
    /// transverse module and pressure angle equal the worm's axial ones, helix angle equals the
    /// worm's lead angle, same hand. Not throated.
    pub fn wheel(&self, teeth: u32) -> SpurGear {
        let g = self.lead_angle();
        SpurGear {
            teeth,
            module: self.module * g.cos(),
            pressure_angle: (self.pressure_angle.to_radians().tan() * g.cos()).atan().to_degrees(),
            helix_angle: g.to_degrees(),
            backlash: self.backlash * g.cos(),
            flank_points: self.flank_points,
        }
    }
}

/// A straight-toothed rack for a spur pinion. The pitch line lies on y = 0 and the teeth point
/// up (+y), the first starting at x = 0; the bar's back is `back` mm below the root.
#[derive(Clone, Debug)]
pub struct Rack {
    pub teeth: u32,
    pub module: f64,
    pub pressure_angle: f64,
    pub backlash: f64,
    /// Material below the root line, mm.
    pub back: f64,
}

pub const MAX_RACK_TEETH: u32 = 500;

impl Rack {
    pub fn validate(&self) -> Result<(), (&'static str, String)> {
        if !(1..=MAX_RACK_TEETH).contains(&self.teeth) {
            return Err(("teeth", format!("must be 1..={MAX_RACK_TEETH}")));
        }
        if !(self.module.is_finite() && self.module >= 0.1 && self.module <= 50.0) {
            return Err(("module", "must be 0.1..=50 mm".into()));
        }
        if !(self.pressure_angle.is_finite() && (10.0..=30.0).contains(&self.pressure_angle)) {
            return Err(("pressure_angle", "must be 10..=30 degrees".into()));
        }
        if !(self.backlash.is_finite() && self.backlash >= 0.0 && self.backlash < self.module / 2.0) {
            return Err(("backlash", "must be at least 0 and smaller than half the module".into()));
        }
        if !(self.back.is_finite() && self.back > 0.0 && self.back <= 10_000.0) {
            return Err(("back", "must be more than 0 and at most 10000 mm".into()));
        }
        Ok(())
    }

    pub fn pitch(&self) -> f64 {
        PI * self.module
    }

    pub fn length(&self) -> f64 {
        self.pitch() * f64::from(self.teeth)
    }

    /// x of tooth `i`'s centre line.
    pub fn tooth_centre(&self, i: u32) -> f64 {
        (f64::from(i) + 0.5) * self.pitch()
    }

    /// Closed outline, counter-clockwise, last point not repeated. Exact: rack flanks are
    /// straight lines at the pressure angle.
    pub fn profile(&self) -> Result<Vec<[f64; 2]>, (&'static str, String)> {
        self.validate()?;
        let m = self.module;
        let tan_a = self.pressure_angle.to_radians().tan();
        // Half the tooth thickness at height y above the pitch line.
        let half = |y: f64| (PI * m / 2.0 - self.backlash) / 2.0 - y * tan_a;
        let (tip, root) = (m, -1.25 * m);
        let (s_tip, s_root) = (half(tip), half(root));
        if s_tip <= 0.0 || 2.0 * s_root >= self.pitch() {
            return Err(("pressure_angle", "the teeth come to a point or close the gaps; use a smaller pressure angle".into()));
        }
        let bottom = root - self.back;
        let mut out = vec![[self.length(), bottom], [self.length(), root]];
        // Along the top from right to left keeps the outline counter-clockwise.
        for i in (0..self.teeth).rev() {
            let x = self.tooth_centre(i);
            out.extend([[x + s_root, root], [x + s_tip, tip], [x - s_tip, tip], [x - s_root, root]]);
        }
        out.extend([[0.0, root], [0.0, bottom]]);
        Ok(out)
    }

    /// The spur pinion that meshes with this rack.
    pub fn pinion(&self, teeth: u32, flank_points: u32) -> SpurGear {
        SpurGear {
            teeth,
            module: self.module,
            pressure_angle: self.pressure_angle,
            helix_angle: 0.0,
            backlash: self.backlash,
            flank_points,
        }
    }

    /// `pinion`'s profile placed in mesh over rack tooth `i`: centre on (tooth centre, pitch
    /// radius), turned so a tooth gap straddles that rack tooth. Returns (points, centre).
    pub fn pinion_in_mesh(&self, pinion: &SpurGear, i: u32) -> Result<(Vec<[f64; 2]>, [f64; 2]), (&'static str, String)> {
        let pts = pinion.profile()?;
        let centre = [self.tooth_centre(i), pinion.circles().pitch];
        // The profile's tooth 0 is on +x, so a gap is centred at π/z; turn it to point down.
        let rot = -PI / 2.0 - PI / f64::from(pinion.teeth);
        let (sin, cos) = rot.sin_cos();
        let pts = pts.iter().map(|p| [p[0] * cos - p[1] * sin + centre[0], p[0] * sin + p[1] * cos + centre[1]]).collect();
        Ok((pts, centre))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gear(teeth: u32) -> SpurGear {
        SpurGear { teeth, module: 2.0, pressure_angle: 20.0, helix_angle: 0.0, backlash: 0.0, flank_points: 8 }
    }

    fn polar(p: [f64; 2]) -> (f64, f64) {
        (p[0].hypot(p[1]), p[1].atan2(p[0]).rem_euclid(2.0 * PI))
    }

    #[test]
    fn circles_match_standard_formulas() {
        let c = gear(20).circles();
        assert!((c.pitch - 20.0).abs() < 1e-12);
        assert!((c.tip - 22.0).abs() < 1e-12);
        assert!((c.root - 17.5).abs() < 1e-12);
        assert!((c.base - 20.0 * 20f64.to_radians().cos()).abs() < 1e-12);
    }

    #[test]
    fn profile_stays_between_root_and_tip_and_is_ccw_monotonic() {
        for z in [6, 12, 17, 20, 41, 42, 60, 150] {
            let g = gear(z);
            let c = g.circles();
            let pts = g.profile().unwrap();
            let mut last = -1.0;
            for (i, &p) in pts.iter().enumerate() {
                let (r, a) = polar(p);
                assert!(r >= c.root - 1e-9 && r <= c.tip + 1e-9, "z={z} point {i} r={r}");
                // Tooth 0's first flank points sit just below angle 0 (wrap to ~2π).
                let a = if i < 12 && a > PI { a - 2.0 * PI } else { a };
                assert!(a >= last - 1e-12, "z={z} angle went backwards at {i}");
                last = a;
            }
        }
    }

    #[test]
    fn tooth_thickness_at_pitch_is_half_the_circular_pitch() {
        let g = SpurGear { flank_points: 40, ..gear(24) };
        let c = g.circles();
        let alpha = g.pressure_angle.to_radians();
        let psi_base = PI / (2.0 * 24.0) + inv(alpha);
        let psi_pitch = psi_base - inv((c.base / c.pitch).acos());
        let arc = 2.0 * psi_pitch * c.pitch;
        assert!((arc - PI * g.module / 2.0).abs() < 1e-9);
    }

    #[test]
    fn backlash_thins_teeth() {
        let a = gear(20).profile().unwrap();
        let b = SpurGear { backlash: 0.1, ..gear(20) }.profile().unwrap();
        // Tip land is narrower with backlash.
        let width = |pts: &[[f64; 2]]| {
            let tip = gear(20).circles().tip;
            let t: Vec<f64> = pts.iter().map(|&p| polar(p)).filter(|(r, a)| (r - tip).abs() < 1e-9 && *a < 0.5).map(|(_, a)| a).collect();
            t.iter().cloned().fold(f64::MIN, f64::max) - t.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!(width(&b) < width(&a));
    }

    #[test]
    fn hostile_inputs_are_refused_not_panicking() {
        for g in [
            SpurGear { teeth: 0, ..gear(20) },
            SpurGear { teeth: 10_000, ..gear(20) },
            SpurGear { module: f64::NAN, ..gear(20) },
            SpurGear { module: -1.0, ..gear(20) },
            SpurGear { pressure_angle: 90.0, ..gear(20) },
            SpurGear { backlash: 5.0, ..gear(20) },
            SpurGear { flank_points: 0, ..gear(20) },
            SpurGear { helix_angle: 60.0, ..gear(20) },
            SpurGear { helix_angle: f64::NAN, ..gear(20) },
        ] {
            assert!(g.profile().is_err(), "{g:?}");
        }
    }

    #[test]
    fn helical_uses_transverse_module_and_normal_tooth_height() {
        let g = SpurGear { helix_angle: 30.0, ..gear(20) };
        let c = g.circles();
        let mt = 2.0 / 30f64.to_radians().cos();
        assert!((c.pitch - mt * 10.0).abs() < 1e-12);
        assert!((c.tip - c.pitch - 2.0).abs() < 1e-12, "addendum is one normal module");
        assert!((c.pitch - c.root - 2.5).abs() < 1e-12, "dedendum is 1.25 normal modules");
        let at = (20f64.to_radians().tan() / 30f64.to_radians().cos()).atan();
        assert!((c.base - c.pitch * at.cos()).abs() < 1e-12);
        // Transverse tooth thickness at the pitch circle is half the transverse circular pitch.
        let psi_base = PI / 40.0 + inv(at);
        let arc = 2.0 * (psi_base - inv((c.base / c.pitch).acos())) * c.pitch;
        assert!((arc - PI * mt / 2.0).abs() < 1e-9);
        // Twist over the face width: b·tanβ / r.
        assert!((g.twist(10.0) - 10.0 * 30f64.to_radians().tan() / c.pitch).abs() < 1e-12);
        assert!(g.profile().is_ok());
    }

    #[test]
    fn zero_helix_is_the_spur_gear() {
        assert_eq!(gear(20).profile().unwrap(), SpurGear { helix_angle: 0.0, ..gear(20) }.profile().unwrap());
        assert_eq!(gear(20).twist(10.0), 0.0);
        assert!(SpurGear { helix_angle: 30.0, ..gear(10) }.min_teeth_without_undercut() < gear(10).min_teeth_without_undercut());
    }

    fn bevel(z: u32, mate: u32) -> BevelGear {
        BevelGear { teeth: z, mate_teeth: mate, module: 2.0, pressure_angle: 20.0, backlash: 0.0, flank_points: 8, face_width: 0.0 }
    }

    fn simple_ccw(pts: &[[f64; 2]], lo: f64, hi: f64) {
        // Star-shaped about the origin: radii within [lo, hi], angle never runs backwards, and
        // the angles add up to one full turn.
        let mut turned = 0.0;
        for (i, w) in pts.windows(2).enumerate() {
            let (r, _) = polar(w[1]);
            assert!(r >= lo - 1e-9 && r <= hi + 1e-9, "point {} r={r}", i + 1);
            let (a0, a1) = (w[0][1].atan2(w[0][0]), w[1][1].atan2(w[1][0]));
            let d = (a1 - a0 + PI).rem_euclid(2.0 * PI) - PI;
            assert!(d >= -1e-12, "angle went backwards at {}", i + 1);
            turned += d;
        }
        let (l, f) = (pts[pts.len() - 1], pts[0]);
        turned += (f[1].atan2(f[0]) - l[1].atan2(l[0]) + PI).rem_euclid(2.0 * PI) - PI;
        assert!((turned - 2.0 * PI).abs() < 1e-9, "turned {turned}");
    }

    #[test]
    fn bevel_cone_matches_standard_formulas() {
        let b = bevel(20, 40);
        let d = 0.5f64.atan();
        assert!((b.pitch_angle() - d).abs() < 1e-12);
        assert!((b.cone_distance() - 20.0 / d.sin()).abs() < 1e-12);
        assert!((b.face() - b.cone_distance() / 3.0).abs() < 1e-12);
        assert!((b.toe_scale() - 2.0 / 3.0).abs() < 1e-12);
        // Apex sits where the shrinking pitch circle reaches zero.
        assert!((b.apex_distance() * (1.0 - b.toe_scale()) - b.axial_length()).abs() < 1e-9);
        let pts = b.profile().unwrap();
        // Heel outside radius r + m·cos δ, root r - 1.25 m·cos δ.
        simple_ccw(&pts, 20.0 - 2.5 * d.cos(), 20.0 + 2.0 * d.cos());
        let rmax = pts.iter().map(|&p| polar(p).0).fold(0.0, f64::max);
        assert!((rmax - (20.0 + 2.0 * d.cos())).abs() < 1e-9);
        // A miter gear (1:1) sits at 45 degrees.
        assert!((bevel(25, 25).pitch_angle() - PI / 4.0).abs() < 1e-12);
        simple_ccw(&bevel(25, 25).profile().unwrap(), 0.0, 100.0);
        simple_ccw(&bevel(12, 60).profile().unwrap(), 0.0, 100.0);
        simple_ccw(&bevel(60, 12).profile().unwrap(), 0.0, 100.0);
    }

    #[test]
    fn bevel_refuses_too_wide_a_face() {
        let b = BevelGear { face_width: 1000.0, ..bevel(20, 20) };
        assert_eq!(b.profile().unwrap_err().0, "face_width");
        assert_eq!(BevelGear { mate_teeth: 2, ..bevel(20, 20) }.profile().unwrap_err().0, "mate_teeth");
    }

    fn worm(starts: u32) -> Worm {
        Worm { starts, module: 2.0, diameter_factor: 10.0, pressure_angle: 20.0, backlash: 0.0, flank_points: 8 }
    }

    #[test]
    fn worm_section_is_the_thread_cut_square_to_the_axis() {
        for starts in 1..=4 {
            let w = worm(starts);
            let c = w.circles();
            let pts = w.section(0.0).unwrap();
            simple_ccw(&pts, c.root, c.tip);
            // At the pitch radius the thread fills exactly half of each start's turn.
            let lead = w.lead();
            assert!((lead - f64::from(starts) * PI * 2.0).abs() < 1e-12);
            let psi_p = PI * (PI * 2.0 / 2.0) / lead;
            assert!((2.0 * psi_p - PI / f64::from(starts)).abs() < 1e-12);
        }
        assert!((worm(1).lead_angle() - 0.1f64.atan()).abs() < 1e-12);
        // Turning the section turns every point.
        let (a, b) = (worm(2).section(0.0).unwrap(), worm(2).section(0.3).unwrap());
        assert!((polar(b[0]).1 - polar(a[0]).1 - 0.3).rem_euclid(2.0 * PI) < 1e-9);
    }

    #[test]
    fn worm_wheel_matches_the_worm_pitch() {
        let w = worm(2);
        let wheel = w.wheel(30);
        // Transverse module and pressure angle of the wheel are the worm's axial ones.
        assert!((wheel.transverse_module() - 2.0).abs() < 1e-12);
        assert!((wheel.transverse_pressure_angle().to_degrees() - 20.0).abs() < 1e-9);
        assert!((wheel.helix_angle - w.lead_angle().to_degrees()).abs() < 1e-12);
        assert!(wheel.profile().is_ok());
    }

    #[test]
    fn worm_refuses_hostile_input() {
        for w in [
            Worm { starts: 0, ..worm(1) },
            Worm { starts: 99, ..worm(1) },
            Worm { diameter_factor: 1.0, ..worm(1) },
            Worm { module: f64::INFINITY, ..worm(1) },
            Worm { pressure_angle: 45.0, ..worm(1) },
            Worm { backlash: 5.0, ..worm(1) },
        ] {
            assert!(w.section(0.0).is_err(), "{w:?}");
        }
    }

    fn rack(teeth: u32) -> Rack {
        Rack { teeth, module: 2.0, pressure_angle: 20.0, backlash: 0.0, back: 5.0 }
    }

    #[test]
    fn rack_teeth_have_the_standard_shape() {
        let r = rack(10);
        let pts = r.profile().unwrap();
        assert_eq!(pts.len(), 4 + 4 * 10);
        // Signed area > 0: counter-clockwise.
        let area: f64 = (0..pts.len()).map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            a[0] * b[1] - b[0] * a[1]
        }).sum::<f64>() / 2.0;
        assert!(area > 0.0);
        // Bar (length × back, below the root line) plus 10 trapezoid teeth of height 2.25 m.
        let tan_a = 20f64.to_radians().tan();
        let (s_tip, s_root) = (PI / 2.0 - 2.0 * tan_a, PI / 2.0 + 2.5 * tan_a);
        let tooth = (s_tip + s_root) * 4.5;
        let bar = 20.0 * PI * 5.0;
        assert!((area - (bar + 10.0 * tooth)).abs() < 1e-9, "{area}");
        // Teeth are half the pitch thick at the pitch line: the flank crosses y = 0 at x ± π m / 4.
        let [x0, y0] = pts[2];
        let [x1, y1] = pts[3];
        let x_at_pitch = x0 + (x1 - x0) * (0.0 - y0) / (y1 - y0);
        assert!((x_at_pitch - (r.tooth_centre(9) + PI / 2.0)).abs() < 1e-12);
        // Flank slope is the pressure angle.
        assert!((((x0 - x1) / (y1 - y0)).atan() - 20f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn pinion_sits_on_the_pitch_line_with_a_gap_over_the_rack_tooth() {
        let r = rack(10);
        fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
            let mut odd = false;
            for i in 0..poly.len() {
                let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
                    odd = !odd;
                }
            }
            odd
        }
        let rack_pts = r.profile().unwrap();
        for z in [8, 12, 13, 20, 40] {
            let g = r.pinion(z, 12);
            let (pts, c) = r.pinion_in_mesh(&g, 5).unwrap();
            assert!((c[1] - g.circles().pitch).abs() < 1e-12);
            // No interference either way.
            for &p in &pts {
                assert!(!inside(&rack_pts, p), "z={z}: pinion point {p:?} inside the rack");
            }
            for &p in &rack_pts {
                assert!(!inside(&pts, p), "z={z}: rack corner {p:?} inside the pinion");
            }
            // And they are in mesh, not just apart: some pinion tooth tip reaches below the
            // rack's tip line (y = m), into a rack gap.
            assert!(pts.iter().any(|p| p[1] < 2.0 - 0.5), "z={z}: pinion does not reach into the rack");
        }
    }

    #[test]
    fn rack_refuses_hostile_input() {
        for r in [
            Rack { teeth: 0, ..rack(1) },
            Rack { teeth: 501, ..rack(1) },
            Rack { module: f64::NAN, ..rack(1) },
            Rack { pressure_angle: 40.0, ..rack(1) },
            Rack { back: 0.0, ..rack(1) },
            Rack { backlash: 2.0, ..rack(1) },
        ] {
            assert!(r.profile().is_err(), "{r:?}");
        }
    }
}

