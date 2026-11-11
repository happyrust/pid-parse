//! The angle a label is lettered at, read the way the plotter states it.
//!
//! A drawing keeps a label's angle as a double, and the angles a person would
//! call level or upright are rarely that to the last bit. The plant backup
//! holds ten labels between 9e-8 and 2e-4 rad off level and two a couple of
//! millionths of a radian off a quarter turn; a fixture holds one more; and
//! of the 470 turned labels on the eleven sheets that have a reference plot,
//! 45 are a few 1e-13 rad short of their quarter turn as this crate reads
//! them.
//!
//! The plotter does not letter at such an angle. `render.dll` hands a label's
//! angle to GDI as `LOGFONT.lfEscapement`, and that field counts whole tenths
//! of a degree: the routine that fills the `LOGFONT` takes `atan2` of the
//! label's matrix, turns it into tenths of a degree and rounds to the nearest
//! whole number, with a matrix whose sine is under 2^-23 short-cut to none.
//! So an angle within a twentieth of a degree of a quarter turn *is* that
//! quarter turn to GDI, and to the PDF driver, which writes the font it is
//! handed: the reference plots state every one of those 470 labels exactly on
//! its quarter turn, in whole numbers (`0 1.012 -1.012 0`).
//!
//! A second routine of the same library, on the way to selecting the font
//! into the device context, states the same product by cutting it to an
//! integer. Read that way the 45 labels would be lettered at 89.9°, which no
//! reference plot shows, so the tenth is taken by rounding. The two readings
//! differ only there and between 0.05° and 0.1° of a quarter turn, where no
//! label of any corpus lies: the farthest of them is 0.012° off.
//!
//! [`of`] reads an angle that way for one purpose, telling a label on a
//! quarter turn from a label turned to some other angle. A label within a
//! twentieth of a degree of a quarter turn is lettered exactly on it. Any
//! other angle is kept as the drawing states it; it is not rounded to its
//! tenth of a degree, which no reference plot has a label to show.

use std::f64::consts::{FRAC_PI_2, PI};

/// The tenths of a degree in a quarter turn.
const QUARTER_TURN_TENTHS: f64 = 900.0;

/// The direction a label's baseline is lettered along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Lettered {
    /// The angle of the baseline, radians counter-clockwise from the page's
    /// x axis: a whole number of quarter turns for a label within a
    /// twentieth of a degree of one, the drawing's own angle otherwise.
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
pub(super) fn of(rotation: f64) -> Lettered {
    let tenths = (rotation.to_degrees() * 10.0).round();
    if !tenths.is_finite() || tenths.rem_euclid(QUARTER_TURN_TENTHS) != 0.0 {
        let (sin, cos) = rotation.sin_cos();
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

    /// An angle a hair off a quarter turn is that quarter turn, with a whole
    /// sine and cosine — which the nearest double to π/2 does not have.
    #[test]
    fn an_angle_a_hair_off_a_quarter_turn_is_the_quarter_turn() {
        let (_, cos) = FRAC_PI_2.sin_cos();
        assert!(cos != 0.0, "the cosine of the double nearest π/2: {cos}");
        for off in [0.0, 2.17e-6, -1.84e-6, 4.14e-13, -8.84e-13] {
            let up = of(FRAC_PI_2 + off);
            assert_eq!((up.sin, up.cos, up.upright), (1.0, 0.0, false), "{off}");
            assert_eq!(up.rotation.to_bits(), FRAC_PI_2.to_bits(), "{off}");
            for down in [of(3.0 * FRAC_PI_2 + off), of(-FRAC_PI_2 + off)] {
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
        }
        // Half a turn runs along the x axis, the other way.
        for rotation in [PI, -PI, PI + 3.0e-6, 3.0 * PI, 5.0 * PI - 1.0e-7] {
            let over = of(rotation);
            assert_eq!(
                (over.sin, over.cos, over.upright),
                (0.0, -1.0, true),
                "{rotation}"
            );
            assert_eq!(over.rotation.to_bits(), PI.to_bits(), "{rotation}");
        }
        // More than a turn is the same direction.
        assert_eq!(of(2.0 * PI + 1.0e-9), of(0.0));
        assert_eq!(of(5.0 * FRAC_PI_2), of(FRAC_PI_2));
    }

    /// The tenth of a degree is the nearest one, not the one below: 0.04° of
    /// either sign is none, 0.06° is a tenth, and a label 1e-13 rad short of
    /// a quarter turn is on it rather than a tenth of a degree back.
    #[test]
    fn the_tenth_of_a_degree_is_the_nearest_one() {
        for degrees in [0.04_f64, -0.04, 0.049, -0.049] {
            assert!(of(degrees.to_radians()).upright, "{degrees}°");
            let turned = of((90.0 + degrees).to_radians());
            assert_eq!((turned.sin, turned.cos), (1.0, 0.0), "{degrees}° off 90°");
        }
        for degrees in [0.051_f64, -0.051, 0.06, -0.06, 0.1, 89.94, 90.06, 179.9] {
            let rotation = degrees.to_radians();
            let kept = of(rotation);
            assert!(!kept.upright, "{degrees}°");
            assert_eq!(kept.rotation.to_bits(), rotation.to_bits(), "{degrees}°");
        }
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
