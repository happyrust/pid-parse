//! The angle a label is lettered at, read the way the plotter states it.
//!
//! A drawing keeps a label's angle as a double, and the angles a person would
//! call level or upright are rarely that to the last bit. The plant backup
//! holds ten labels between 9e-8 and 2e-4 rad off level and two a couple of
//! millionths of a radian off a quarter turn; a fixture holds one more; and
//! of the 470 turned labels on the eleven sheets that have a reference plot,
//! 45 are short of their quarter turn as this crate reads them — seven by a
//! bit or two of the double, the rest by up to 9e-13 rad.
//!
//! The plotter does not letter at the angle a hair off level. `render.dll` hands GDI a
//! font whose `LOGFONT.lfEscapement` counts whole tenths of a degree. Two
//! routines of `RADInstallA~\render.dll` make that number from `atan2` of the
//! label's matrix, times `-180/π`, times ten. `sub_1000B1F0` rounds to the
//! nearest tenth, and a matrix whose sine is under 2^-23 is written `0`
//! whatever the cosine, so a half turn is level. `sub_10080B50` cuts the same
//! product toward zero (`cvttsd2si`); the same cutoff returns `1800` when the
//! cosine is negative and `0` otherwise, so a half turn is upside down. The
//! text path calls `sub_100808F0` (`0x1007F107`, `0x1008040E`), which passes
//! that cut integer to `sub_1007EBA0`. The builder writes it over
//! `lfEscapement` and `lfOrientation` after the virtual call that fills the
//! `LOGFONT`, and `CreateFontIndirectA` (`0x1001B57D`, the only call) is
//! handed that `LOGFONT`. The rounding routine's thunk is not taken up by
//! any call. The tenth that reaches GDI is the one cut toward zero.
//!
//! # What the reference plots show
//!
//! - Every one of their 470 turned labels is stated exactly on its quarter
//!   turn, in whole numbers (`0 1.012 -1.012 0`). Of ours, the 425 that are
//!   not short of it are too. The 45 that are short lie within 1e-12 rad of
//!   the quarter and are kept at the angle the drawing states. At 89.9° the
//!   longest of them, 23 device units, would stand 0.04 of a unit aside, on
//!   the same whole unit, and the driver's 256ths ([`super::text_matrix`])
//!   would still write `0 1.012 -1.012 0`. Those plots cannot tell the cut
//!   from the quarter, and lettering them at the cut tenth moves a centred
//!   label on one of the sheets past the place the plot gives it.
//! - A label lettered level looks as the ten of the backup do under this
//!   rule: an upright Arial at `Tf 12.45` or Arial Narrow at `Tf 10.17` under
//!   `1 0 0 1`, not the same face 1.2 % or 0.8 % larger under the matrix of a
//!   turned label.
//!
//! # What the factory plot of the long label shows
//!
//! `25-WO-0103528-B2-EH40` on the plant backup's `低温分离器C` is 1.06e-4°
//! short of a right angle and 296 device units long. The Amyuni plot of that
//! sheet (PDF Converter 4.5.2.9) letters the first nineteen glyphs on one
//! vertical axis and the last two one device unit aside of it: 0.48 pt,
//! 25.4/150 mm. Rounding the tenth would put every glyph on the axis. The
//! cut tenth is 89.9°. A run stands half a unit aside at 286.5 units, so
//! this one, being longer, stands its last glyph on the next unit. The same
//! sheet's `40-WO-0103526-B2-EH40` is 1.25e-4° past the right angle; the cut
//! tenth is 900 and every glyph of that label is on the axis. The other 223
//! labels the two readings part on are 284 units long or less. Two fixture
//! labels sit at exactly half a turn: the cut reads 1800 and the rounding
//! routine reads 0, and there is no factory plot of them either.
//!
//! [`of`] reads an angle that way for one purpose, telling a label on a
//! quarter turn from a label turned to some other angle. The tenth is cut
//! toward zero. A label whose cut tenth is a multiple of 900 is lettered on
//! that quarter turn: one that has passed it by less than a tenth of a
//! degree, and one either side of level by less than a tenth. A label whose
//! nearest tenth is a right angle, whose cut tenth falls short of it, and
//! whose own angle is short of that right angle by more than a float hair,
//! is lettered at the cut tenth — the factory plot is that case. A float
//! hair short of the quarter keeps the angle the drawing states. Any other
//! angle is kept as the drawing states it; it is not cut to its tenth of a
//! degree.

use std::f64::consts::{FRAC_PI_2, PI};

/// The tenths of a degree in a quarter turn.
const QUARTER_TURN_TENTHS: f64 = 900.0;

/// A shortfall this close to a right angle keeps the drawing's own angle.
///
/// The 45 labels on the eleven reference plots lie within 1e-12 rad of the
/// quarter. `25-WO-0103528-B2-EH40` lies 1.84e-6 rad short, which the factory
/// plot letters at the cut tenth.
const HAIR_SHORT_OF_A_RIGHT_ANGLE: f64 = 1.0e-9;

/// The direction a label's baseline is lettered along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Lettered {
    /// The angle of the baseline, radians counter-clockwise from the page's
    /// x axis: a whole number of quarter turns when the tenth cut toward
    /// zero is a multiple of 900, that tenth when it falls just short of a
    /// right angle, the drawing's own angle otherwise.
    pub(super) rotation: f64,
    /// Sine of [`Self::rotation`], exact on a quarter turn.
    pub(super) sin: f64,
    /// Cosine of [`Self::rotation`], exact on a quarter turn.
    pub(super) cos: f64,
    /// Whether the baseline runs along the page's x axis, either way: what
    /// the writer sets as a string on whole-pixel starts rather than glyph
    /// by glyph.
    pub(super) upright: bool,
}

/// The direction a label at `rotation` is lettered along.
///
/// The tenth of a degree is cut toward zero, as `render.dll` `sub_10080B50`
/// does for the font `CreateFontIndirectA` is handed. A sine under 2^-23 is
/// `0` or `1800` from the sign of the cosine, which is that routine's cutoff.
/// A right angle the cut falls short of is lettered at that tenth: the
/// factory plot of `25-WO-0103528-B2-EH40` stands its last glyphs one device
/// unit aside of the axis rounding would leave them on.
pub(super) fn of(rotation: f64) -> Lettered {
    let (sin, cos) = rotation.sin_cos();
    // `2^-23`, the cutoff `sub_10080B50` compares the matrix sine with.
    let tiny = sin.abs() < 1.0 / 8388608.0;
    let degrees = rotation.to_degrees();
    let tenths = if tiny {
        if cos < 0.0 { 1800.0 } else { 0.0 }
    } else {
        (degrees * 10.0).trunc()
    };
    if !tenths.is_finite() {
        return Lettered {
            rotation,
            sin,
            cos,
            upright: false,
        };
    }
    // The nearest tenth is a right angle and the cut tenth is the one short
    // of it. GDI is handed the cut, so the label is lettered there.
    let nearest = if tiny {
        tenths
    } else {
        (degrees * 10.0).round()
    };
    let right = (nearest / 10.0).to_radians();
    if nearest.is_finite()
        && nearest.rem_euclid(2.0 * QUARTER_TURN_TENTHS) == QUARTER_TURN_TENTHS
        && tenths.rem_euclid(QUARTER_TURN_TENTHS) != 0.0
        && (rotation - right).abs() >= HAIR_SHORT_OF_A_RIGHT_ANGLE
    {
        let rotation = (tenths / 10.0).to_radians();
        let (sin, cos) = rotation.sin_cos();
        return Lettered {
            rotation,
            sin,
            cos,
            upright: false,
        };
    }
    if tenths.rem_euclid(QUARTER_TURN_TENTHS) != 0.0 {
        return Lettered {
            rotation,
            sin,
            cos,
            upright: false,
        };
    }
    // The quarter turn in 0..4. Its sine and cosine are whole numbers, which
    // `sin_cos` of the nearest double to a multiple of π/2 does not return.
    let quarter = (tenths / QUARTER_TURN_TENTHS).rem_euclid(4.0);
    let (rotation, sin, cos) = if quarter < 0.5 {
        (0.0, 0.0, 1.0)
    } else if quarter < 1.5 {
        (FRAC_PI_2, 1.0, 0.0)
    } else if quarter < 2.5 {
        (PI, 0.0, -1.0)
    } else {
        (3.0 * FRAC_PI_2, -1.0, 0.0)
    };
    Lettered {
        rotation,
        sin,
        cos,
        upright: sin == 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{Lettered, of};
    use std::f64::consts::{FRAC_PI_2, PI};

    /// The angles the corpora hold a hair off level are level: sine nought
    /// and cosine one, exactly.
    #[test]
    fn an_angle_a_hair_off_level_is_level() {
        for rotation in [0.0, 9.0e-8, -2.59e-6, 2.0e-4, -2.016e-4, 2.22e-16] {
            assert_eq!(
                of(rotation),
                Lettered {
                    rotation: 0.0,
                    sin: 0.0,
                    cos: 1.0,
                    upright: true,
                },
                "{rotation} rad"
            );
        }
    }

    /// An angle that has passed a quarter turn, and is still inside the tenth
    /// the cut keeps, is that quarter turn. A shortfall is not: rounding
    /// would put it on the quarter.
    #[test]
    fn an_angle_past_a_quarter_turn_is_the_quarter_turn() {
        let (_, cos) = FRAC_PI_2.sin_cos();
        assert!(cos != 0.0, "the cosine of the double nearest π/2: {cos}");
        // Past +π/2 and +3π/2, away from zero.
        for off in [0.0, 2.17e-6, 4.14e-13] {
            let up = of(FRAC_PI_2 + off);
            assert_eq!((up.sin, up.cos, up.upright), (1.0, 0.0, false), "{off}");
            assert_eq!(up.rotation.to_bits(), FRAC_PI_2.to_bits(), "{off}");
            let down = of(3.0 * FRAC_PI_2 + off);
            assert_eq!(
                (down.sin, down.cos, down.upright),
                (-1.0, 0.0, false),
                "{off}"
            );
            assert_eq!(
                down.rotation.to_bits(),
                (3.0 * FRAC_PI_2).to_bits(),
                "{off}"
            );
        }
        // Short of +π/2 by the factory label's shortfall: the cut tenth is
        // 89.9°. A float hair short of it keeps the drawing's angle. The
        // same shortfall past −π/2, away from zero, is still the downward
        // quarter.
        let cut_up = (899.0_f64 / 10.0).to_radians();
        let factory = FRAC_PI_2 - 1.84e-6;
        assert_eq!(of(factory).rotation.to_bits(), cut_up.to_bits());
        let hair = FRAC_PI_2 - 8.84e-13;
        assert_eq!(of(hair).rotation.to_bits(), hair.to_bits());
        let down = of(-FRAC_PI_2 - 8.84e-13);
        assert_eq!((down.sin, down.cos, down.upright), (-1.0, 0.0, false));
        assert_eq!(down.rotation.to_bits(), (3.0 * FRAC_PI_2).to_bits());
        // Exactly on −π/2, and a step back toward zero. The cut tenth of
        // that step is −89.9°, not the raw angle and not the quarter.
        let down = of(-FRAC_PI_2);
        assert_eq!((down.sin, down.cos), (-1.0, 0.0));
        assert_eq!(down.rotation.to_bits(), (3.0 * FRAC_PI_2).to_bits());
        let toward = -FRAC_PI_2 + 2.17e-6;
        let cut_down = (-899.0_f64 / 10.0).to_radians();
        assert_eq!(of(toward).rotation.to_bits(), cut_down.to_bits());
        // Half a turn runs along the x axis, the other way. `5π − 1e-7` is
        // inside the sine cutoff, so the routine returns 1800; the cut of
        // the product is 8999.
        for rotation in [PI, -PI, PI + 3.0e-6, 3.0 * PI, 5.0 * PI - 1.0e-7] {
            let over = of(rotation);
            assert_eq!(
                (over.sin, over.cos, over.upright),
                (0.0, -1.0, true),
                "{rotation}"
            );
            assert_eq!(over.rotation.to_bits(), PI.to_bits(), "{rotation}");
        }
        // Just outside the cutoff, short of a half turn. Rounding calls this
        // a half turn.
        let shy = PI - 2.0e-7;
        assert_eq!(of(shy).rotation.to_bits(), shy.to_bits());
        // More than a turn is the same direction.
        assert_eq!(of(2.0 * PI + 1.0e-9), of(0.0));
        assert_eq!(of(5.0 * FRAC_PI_2), of(FRAC_PI_2));
    }

    /// The tenth of a degree is cut toward zero, not taken to the nearest.
    /// `0.06°` is level, `90.06°` is a quarter turn, and `89.96°` is the cut
    /// tenth 89.9° — the nearest tenth would put it on the quarter. Each of
    /// those is the other way when the tenth is the nearest one.
    #[test]
    fn the_tenth_of_a_degree_is_cut_toward_zero() {
        for degrees in [0.04_f64, -0.04, 0.06, -0.06, 0.099, -0.099] {
            assert!(of(degrees.to_radians()).upright, "{degrees}°");
        }
        for degrees in [90.0_f64, 90.04, 90.06, 90.099] {
            let turned = of(degrees.to_radians());
            assert_eq!(
                (turned.sin, turned.cos, turned.upright),
                (1.0, 0.0, false),
                "{degrees}°"
            );
        }
        for degrees in [180.0_f64, 180.06] {
            let turned = of(degrees.to_radians());
            assert_eq!(
                (turned.sin, turned.cos, turned.upright),
                (0.0, -1.0, true),
                "{degrees}°"
            );
        }
        let down = of((-90.06_f64).to_radians());
        assert_eq!((down.sin, down.cos, down.upright), (-1.0, 0.0, false));
        for degrees in [0.1_f64, -0.1, 90.1, 179.96] {
            let rotation = degrees.to_radians();
            let kept = of(rotation);
            assert!(!kept.upright, "{degrees}°");
            assert_eq!(kept.rotation.to_bits(), rotation.to_bits(), "{degrees}°");
        }
        let cut_up = (899.0_f64 / 10.0).to_radians();
        let cut_down = (-899.0_f64 / 10.0).to_radians();
        assert_eq!(
            of(89.96_f64.to_radians()).rotation.to_bits(),
            cut_up.to_bits()
        );
        assert_eq!(
            of((-89.96_f64).to_radians()).rotation.to_bits(),
            cut_down.to_bits()
        );
        // A float hair short of the quarter keeps the drawing's angle.
        let short = FRAC_PI_2 - 1.0e-13;
        assert_eq!(of(short).rotation.to_bits(), short.to_bits());
    }

    /// `25-WO-0103528-B2-EH40` on the Amyuni plot of `低温分离器C`.
    ///
    /// The label is 1.056568704740e-4° short of a right angle and 296 device
    /// units long. The plot puts the last two glyphs 0.48 pt aside of the
    /// axis the first nineteen share: one device unit, 25.4/150 mm. The cut
    /// tenth is 89.9°, and 296 units at that angle stands just over half a
    /// unit aside, which rounds to that same device unit. Rounding the tenth
    /// lands on the quarter and leaves the last glyph on the axis.
    #[test]
    fn the_factory_plot_stands_the_last_glyphs_a_device_unit_aside() {
        let degrees: f64 = 90.0 - 1.056568704740e-4;
        let lettered = of(degrees.to_radians());
        let cut = (899.0_f64 / 10.0).to_radians();
        assert_eq!(lettered.rotation.to_bits(), cut.to_bits());
        let aside = 296.0 * (FRAC_PI_2 - lettered.rotation).sin();
        assert_eq!(aside.round(), 1.0, "the cut stands {aside} units aside");
        let raw = 296.0 * (1.056568704740e-4_f64.to_radians()).sin();
        assert!(
            raw.round() == 0.0 && raw < 0.001,
            "the drawing's own angle stands only {raw} units aside"
        );
        let factory_mm: f64 = 0.48 * 25.4 / 72.0;
        assert!((factory_mm - 25.4 / 150.0).abs() < 1e-12, "{factory_mm}");
    }

    /// An angle that is no quarter turn keeps its value, sine and cosine: it
    /// is not rounded to its tenth of a degree.
    #[test]
    fn an_angle_that_is_no_quarter_turn_keeps_its_value() {
        for degrees in [37.26_f64, -12.5, 45.0, 135.049, 301.0, 1234.5] {
            let rotation = degrees.to_radians();
            let (sin, cos) = rotation.sin_cos();
            assert_eq!(
                of(rotation),
                Lettered {
                    rotation,
                    sin,
                    cos,
                    upright: false,
                },
                "{degrees}°"
            );
        }
    }

    /// An angle that is no number is handed back as it came, and one too
    /// large to have a tenth of a degree still reads as a direction.
    #[test]
    fn an_angle_that_is_no_number_is_not_a_quarter_turn() {
        for rotation in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let read = of(rotation);
            assert!(!read.upright, "{rotation}");
            assert_eq!(read.rotation.to_bits(), rotation.to_bits(), "{rotation}");
        }
        for rotation in [f64::MAX, f64::MIN, 1.0e300, -1.0e300] {
            let read = of(rotation);
            assert!(
                (read.sin.hypot(read.cos) - 1.0).abs() < 1e-9,
                "{rotation}: {read:?}"
            );
        }
    }
}
