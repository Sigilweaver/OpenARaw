//! Q-TOF flight-time to m/z calibration.
//!
//! Q-TOF centroid x-values (`MSPeak.bin`, FormatID=2) and the profile axis
//! (`MSProfile.bin`, FormatID=1) are flight times, not m/z. A calibration
//! has two steps (see `docs/format/05-msmasscal.md`):
//!
//! ```text
//! m/z = (A * (t - T0))^2 - sum_k c_k * clamp(t, t_min, t_max)^k
//! ```
//!
//! The first term is the `Traditional` step. The sum is the `Polynomial`
//! step: its eight values are `t_min`, `t_max` and six coefficient slots.
//! The powers `k` are the set bits of the step's `ValueUseFlags` (from
//! `DefaultMassCal.xml`), lowest bit first; coefficient slot `j` belongs to
//! the `j`-th set bit.

/// Where a scan's calibration came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CalibrationSource {
    /// The scan's own `MSMassCal.bin` record (reference-mass corrected).
    MassCalRecord,
    /// The bundle's `DefaultMassCal.xml` entry for the scan's
    /// `CalibrationID`.
    DefaultMassCal,
}

impl CalibrationSource {
    /// Stable lowercase label: `"mass_cal_record"` or `"default_mass_cal"`.
    pub fn as_str(self) -> &'static str {
        match self {
            CalibrationSource::MassCalRecord => "mass_cal_record",
            CalibrationSource::DefaultMassCal => "default_mass_cal",
        }
    }
}

/// Number of values in a `Polynomial` step: `t_min`, `t_max` and six
/// coefficient slots.
pub const POLYNOMIAL_VALUES: usize = 8;

/// The `Polynomial` correction step.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PolynomialStep {
    /// Lower end of the flight-time range the polynomial is evaluated on.
    pub t_min: f64,
    /// Upper end of the flight-time range the polynomial is evaluated on.
    pub t_max: f64,
    /// `(power, coefficient)` pairs.
    pub terms: Vec<(u32, f64)>,
}

impl PolynomialStep {
    /// Build the step from its `ValueUseFlags` and its eight values
    /// (`[t_min, t_max, c_0..c_5]`).
    ///
    /// Fails if the value count is wrong, any value is not finite,
    /// `t_min > t_max`, more powers are flagged than there are coefficient
    /// slots, or a coefficient slot without a flagged power is non-zero.
    pub fn from_flags(flags: u32, values: &[f64]) -> crate::Result<Self> {
        if values.len() != POLYNOMIAL_VALUES {
            return Err(crate::Error::Parse(format!(
                "Polynomial calibration step needs {POLYNOMIAL_VALUES} values, got {}",
                values.len()
            )));
        }
        if values.iter().any(|v| !v.is_finite()) {
            return Err(crate::Error::Parse(format!(
                "Polynomial calibration step has a non-finite value: {values:?}"
            )));
        }
        let (t_min, t_max) = (values[0], values[1]);
        if t_min > t_max {
            return Err(crate::Error::Parse(format!(
                "Polynomial calibration range is inverted: {t_min} > {t_max}"
            )));
        }
        let coefficients = &values[2..];
        let powers: Vec<u32> = (0..u32::BITS).filter(|k| flags >> k & 1 == 1).collect();
        if powers.len() > coefficients.len() {
            return Err(crate::Error::Parse(format!(
                "Polynomial ValueUseFlags {flags} selects {} powers; at most {} coefficients exist",
                powers.len(),
                coefficients.len()
            )));
        }
        if let Some(v) = coefficients[powers.len()..].iter().find(|v| **v != 0.0) {
            return Err(crate::Error::Parse(format!(
                "Polynomial coefficient {v} has no power in ValueUseFlags {flags}"
            )));
        }
        let terms = powers
            .into_iter()
            .zip(coefficients.iter().copied())
            .collect();
        Ok(PolynomialStep {
            t_min,
            t_max,
            terms,
        })
    }

    /// The correction subtracted from the `Traditional` m/z at flight
    /// time `t`.
    pub fn correction(&self, t: f64) -> f64 {
        let t = t.clamp(self.t_min, self.t_max);
        self.terms
            .iter()
            .map(|&(power, c)| c * t.powi(power as i32))
            .sum()
    }
}

/// One scan's flight-time calibration.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TofCalibration {
    /// `Traditional` step slope `A`.
    pub a: f64,
    /// `Traditional` step time offset `T0`, in flight-time units.
    pub t0: f64,
    /// `Polynomial` correction step.
    pub polynomial: PolynomialStep,
    /// Where the calibration came from.
    pub source: CalibrationSource,
}

impl TofCalibration {
    /// Build a calibration from a ten-value coefficient list
    /// (`[A, T0, t_min, t_max, c_0..c_5]`, the layout of an `MSMassCal.bin`
    /// record) and the `Polynomial` step's `ValueUseFlags`.
    pub fn from_coefficients(
        coefficients: &[f64],
        polynomial_flags: u32,
        source: CalibrationSource,
    ) -> crate::Result<Self> {
        if coefficients.len() != 2 + POLYNOMIAL_VALUES {
            return Err(crate::Error::Parse(format!(
                "mass calibration needs {} coefficients, got {}",
                2 + POLYNOMIAL_VALUES,
                coefficients.len()
            )));
        }
        let (a, t0) = (coefficients[0], coefficients[1]);
        if !a.is_finite() || !t0.is_finite() || a == 0.0 {
            return Err(crate::Error::Parse(format!(
                "invalid Traditional calibration coefficients A={a}, T0={t0}"
            )));
        }
        Ok(TofCalibration {
            a,
            t0,
            polynomial: PolynomialStep::from_flags(polynomial_flags, &coefficients[2..])?,
            source,
        })
    }

    /// Convert one flight time to m/z.
    pub fn mz(&self, t: f64) -> f64 {
        let r = self.a * (t - self.t0);
        r * r - self.polynomial.correction(t)
    }

    /// Convert flight times to m/z in place.
    pub fn apply(&self, x: &mut [f64]) {
        for v in x.iter_mut() {
            *v = self.mz(*v);
        }
    }
}

/// Test helpers shared with the decoder and reader tests.
#[cfg(test)]
impl TofCalibration {
    /// A calibration with an empty `Polynomial` step.
    pub(crate) fn traditional_only(a: f64, t0: f64) -> Self {
        let mut c = [0.0; 2 + POLYNOMIAL_VALUES];
        c[0] = a;
        c[1] = t0;
        TofCalibration::from_coefficients(&c, 0, CalibrationSource::MassCalRecord).unwrap()
    }

    /// Inverse of [`TofCalibration::mz`] for a calibration without
    /// polynomial terms, on the `t >= T0` branch.
    pub(crate) fn flight_time(&self, mz: f64) -> f64 {
        assert!(self.polynomial.terms.is_empty());
        mz.sqrt() / self.a + self.t0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DefaultCalibrationID="1"` of
    /// `PXD001310/2013-02-26-001 EF CR DMSO 60m r1.d/AcqData/DefaultMassCal.xml`.
    const PXD001310_DEFAULT: [f64; 10] = [
        0.000347994641277165,
        1018.75777725805,
        32245.5711007443,
        113125.231188769,
        1.94638932212931E-17,
        -1.75610630976578E-26,
        2.70160202926823E-31,
        -1.13530736865671E-36,
        0.0,
        0.0,
    ];
    const PXD001310_FLAGS: u32 = 232;

    fn pxd001310() -> TofCalibration {
        TofCalibration::from_coefficients(
            &PXD001310_DEFAULT,
            PXD001310_FLAGS,
            CalibrationSource::DefaultMassCal,
        )
        .unwrap()
    }

    /// First scan of that bundle (ScanID 2271): the most intense centroid
    /// has flight time 66165.7306301066 and `MSScan.bin` stores
    /// `BasePeakMZ` 513.9653233122384.
    #[test]
    fn reproduces_stored_base_peak_of_real_scan() {
        let mz = pxd001310().mz(66165.7306301066);
        assert!(
            ((mz - 513.9653233122384) / 513.9653233122384).abs() < 1e-9,
            "{mz}"
        );
    }

    #[test]
    fn flags_select_powers_in_bit_order() {
        // 232 = bits 3, 5, 6, 7.
        let step = &pxd001310().polynomial;
        assert_eq!(
            step.terms.iter().map(|t| t.0).collect::<Vec<_>>(),
            vec![3, 5, 6, 7]
        );
        assert_eq!(step.terms[0].1, 1.94638932212931E-17);
    }

    #[test]
    fn polynomial_is_clamped_to_its_range() {
        let step = &pxd001310().polynomial;
        assert_eq!(step.correction(1000.0), step.correction(step.t_min));
        assert_eq!(step.correction(1e9), step.correction(step.t_max));
        assert_ne!(step.correction(50000.0), step.correction(step.t_min));
    }

    #[test]
    fn zero_flags_mean_no_correction() {
        let mut c = PXD001310_DEFAULT;
        c[4..].fill(0.0);
        let cal =
            TofCalibration::from_coefficients(&c, 0, CalibrationSource::MassCalRecord).unwrap();
        assert!(cal.polynomial.terms.is_empty());
        let t = 60000.0;
        assert_eq!(cal.mz(t), (c[0] * (t - c[1])).powi(2));
    }

    #[test]
    fn rejects_invalid_coefficients() {
        let src = CalibrationSource::MassCalRecord;
        assert!(TofCalibration::from_coefficients(&[], 0, src).is_err());
        assert!(TofCalibration::from_coefficients(&PXD001310_DEFAULT[..2], 0, src).is_err());
        let mut c = PXD001310_DEFAULT;
        c[0] = 0.0;
        assert!(TofCalibration::from_coefficients(&c, PXD001310_FLAGS, src).is_err());
        let mut c = PXD001310_DEFAULT;
        c[1] = f64::NAN;
        assert!(TofCalibration::from_coefficients(&c, PXD001310_FLAGS, src).is_err());
    }

    #[test]
    fn rejects_flags_that_do_not_match_coefficients() {
        let values = &PXD001310_DEFAULT[2..];
        // Seven powers but only six slots.
        assert!(PolynomialStep::from_flags(0b111_1111, values).is_err());
        // Four non-zero coefficients but only three powers.
        let err = PolynomialStep::from_flags(0b111, values).unwrap_err();
        assert!(err.to_string().contains("has no power"));
        // Inverted range.
        let mut v = values.to_vec();
        v.swap(0, 1);
        assert!(PolynomialStep::from_flags(PXD001310_FLAGS, &v).is_err());
    }
}
