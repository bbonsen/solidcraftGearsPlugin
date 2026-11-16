//! Involute spur gear geometry. Pure math, no host calls, so it is tested natively.

use std::f64::consts::PI;

/// Inputs of a standard (unshifted, full-depth) involute spur gear. Lengths in mm.
#[derive(Clone, Debug)]
pub struct SpurGear {
    pub teeth: u32,
    pub module: f64,
    /// Pressure angle in degrees.
    pub pressure_angle: f64,
    /// Circular backlash at the pitch circle, removed from tooth thickness (mm).
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
        if !(2..=40).contains(&self.flank_points) {
            return Err(("flank_points", "must be 2..=40".into()));
        }
        Ok(())
    }

    pub fn circles(&self) -> Circles {
        let pitch = self.module * f64::from(self.teeth) / 2.0;
        Circles {
            pitch,
            base: pitch * self.pressure_angle.to_radians().cos(),
            tip: pitch + self.module,
            root: pitch - 1.25 * self.module,
        }
    }

    /// Fewest teeth that avoid undercut for this pressure angle (2 / sin²α).
    pub fn min_teeth_without_undercut(&self) -> f64 {
        2.0 / self.pressure_angle.to_radians().sin().powi(2)
    }

    /// Closed profile, counter-clockwise, gear centred on the origin with tooth 0 on +X.
    /// The last point is not repeated; the caller closes the loop.
    pub fn profile(&self) -> Result<Vec<[f64; 2]>, (&'static str, String)> {
        self.validate()?;
        let c = self.circles();
        let z = f64::from(self.teeth);
        let alpha = self.pressure_angle.to_radians();

        // Half the tooth's angular thickness measured at the base circle.
        let half_pitch = (PI * self.module / 2.0 - self.backlash) / (2.0 * c.pitch);
        let psi_base = half_pitch + inv(alpha);
        // Half angle of the tooth at radius r (r >= base).
        let psi = |r: f64| psi_base - inv((c.base / r).clamp(-1.0, 1.0).acos());

        let psi_tip = psi(c.tip);
        if psi_tip <= 0.0 {
            return Err(("teeth", "teeth come to a point at the tip circle; use more teeth or a smaller pressure angle".into()));
        }

        // Flank radii from the root (or base, whichever is larger) up to the tip.
        let start = c.root.max(c.base);
        let n = self.flank_points as usize;
        let flank: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let t = i as f64 / (n - 1) as f64;
                // Cluster points near the base where curvature is highest.
                let r = start + (c.tip - start) * t * t;
                (r, psi(r))
            })
            .collect();
        let psi_root_flank = flank[0].1;

        let pt = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
        let step = 2.0 * PI / z;
        let mut out = Vec::with_capacity(self.teeth as usize * (2 * n + 6));
        for k in 0..self.teeth {
            let th = step * f64::from(k);
            // Radial segment below the base circle, when the root lies inside it.
            if c.root < c.base {
                out.push(pt(c.root, th - psi_base));
            }
            for &(r, p) in &flank {
                out.push(pt(r, th - p));
            }
            // Tip land.
            for j in 1..3 {
                let a = -psi_tip + 2.0 * psi_tip * f64::from(j) / 3.0;
                out.push(pt(c.tip, th + a));
            }
            for &(r, p) in flank.iter().rev() {
                out.push(pt(r, th + p));
            }
            let gap_start = if c.root < c.base {
                out.push(pt(c.root, th + psi_base));
                psi_base
            } else {
                psi_root_flank
            };
            // Root land between this tooth and the next.
            let gap = step - 2.0 * gap_start;
            for j in 1..3 {
                out.push(pt(c.root, th + gap_start + gap * f64::from(j) / 3.0));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gear(teeth: u32) -> SpurGear {
        SpurGear { teeth, module: 2.0, pressure_angle: 20.0, backlash: 0.0, flank_points: 8 }
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
        ] {
            assert!(g.profile().is_err(), "{g:?}");
        }
    }
}
