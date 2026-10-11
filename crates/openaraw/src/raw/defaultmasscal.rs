//! DefaultMassCal.xml parser.
//!
//! Each `DefaultCalibration` (keyed by `DefaultCalibrationID`, which
//! `MSScan.bin` records reference through `CalibrationID`) holds a
//! `Traditional` step (2 values) and a `Polynomial` step (8 values plus
//! `ValueUseFlags`). See `docs/format/05-msmasscal.md`.

use crate::raw::calibration::{CalibrationSource, TofCalibration, POLYNOMIAL_VALUES};
use std::path::Path;

/// One `DefaultCalibration` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct DefaultCalibration {
    /// `DefaultCalibrationID`.
    pub id: i32,
    /// `Traditional` step values `[A, T0]`.
    pub traditional: [f64; 2],
    /// `Polynomial` step `ValueUseFlags`.
    pub polynomial_flags: u32,
    /// `Polynomial` step values `[t_min, t_max, c_0..c_5]`.
    pub polynomial: [f64; POLYNOMIAL_VALUES],
}

impl DefaultCalibration {
    /// The calibration as a [`TofCalibration`].
    pub fn to_tof(&self) -> crate::Result<TofCalibration> {
        let mut c = Vec::with_capacity(2 + POLYNOMIAL_VALUES);
        c.extend_from_slice(&self.traditional);
        c.extend_from_slice(&self.polynomial);
        TofCalibration::from_coefficients(
            &c,
            self.polynomial_flags,
            CalibrationSource::DefaultMassCal,
        )
    }
}

pub fn parse(path: &Path) -> crate::Result<Vec<DefaultCalibration>> {
    let text = std::fs::read_to_string(path)?;
    parse_str(&text)
}

fn err(msg: impl Into<String>) -> crate::Error {
    crate::Error::Parse(format!("DefaultMassCal.xml: {}", msg.into()))
}

fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, tag: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn child_text<'a>(node: roxmltree::Node<'a, '_>, tag: &str) -> crate::Result<&'a str> {
    child(node, tag)
        .and_then(|n| n.text())
        .map(str::trim)
        .ok_or_else(|| err(format!("<{}> has no <{tag}>", node.tag_name().name())))
}

fn parse_num<T: std::str::FromStr>(text: &str, what: &str) -> crate::Result<T> {
    text.parse()
        .map_err(|_| err(format!("{what} {text:?} is not a number")))
}

/// One `<Step>`: formula, flags and values in `Number` order.
struct Step {
    formula: String,
    flags: u32,
    values: Vec<f64>,
}

fn parse_step(step: roxmltree::Node) -> crate::Result<Step> {
    let formula = child_text(step, "CalibrationFormula")?.to_string();
    let flags = parse_num(child_text(step, "ValueUseFlags")?, "ValueUseFlags")?;
    let count: usize = parse_num(
        child_text(step, "NumberOfCoefficients")?,
        "NumberOfCoefficients",
    )?;
    let values_node =
        child(step, "Values").ok_or_else(|| err(format!("{formula} step has no <Values>")))?;
    let mut values = vec![None; count];
    for v in values_node.children().filter(|n| n.has_tag_name("Value")) {
        let number: usize = parse_num(v.attribute("Number").unwrap_or(""), "Value Number")?;
        let slot = number
            .checked_sub(1)
            .and_then(|i| values.get_mut(i))
            .ok_or_else(|| err(format!("{formula} Value Number {number} out of range")))?;
        *slot = Some(parse_num::<f64>(v.text().unwrap_or("").trim(), "Value")?);
    }
    let values = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| v.ok_or_else(|| err(format!("{formula} Value {} missing", i + 1))))
        .collect::<crate::Result<_>>()?;
    Ok(Step {
        formula,
        flags,
        values,
    })
}

/// Parse DefaultMassCal.xml text. Any structural problem (missing step,
/// wrong value count, unparseable number, duplicate ID) is an error.
pub fn parse_str(text: &str) -> crate::Result<Vec<DefaultCalibration>> {
    let text = text.trim_start_matches('\u{feff}');
    let doc = roxmltree::Document::parse(text).map_err(|e| err(e.to_string()))?;
    let mut out: Vec<DefaultCalibration> = Vec::new();
    for cal in doc
        .descendants()
        .filter(|n| n.has_tag_name("DefaultCalibration"))
    {
        let id: i32 = parse_num(
            cal.attribute("DefaultCalibrationID").unwrap_or(""),
            "DefaultCalibrationID",
        )?;
        if out.iter().any(|c| c.id == id) {
            return Err(err(format!("duplicate DefaultCalibrationID {id}")));
        }
        let steps = cal
            .children()
            .filter(|n| n.has_tag_name("Step"))
            .map(parse_step)
            .collect::<crate::Result<Vec<_>>>()?;
        let find = |formula: &str, len: usize| -> crate::Result<&Step> {
            let mut matching = steps.iter().filter(|s| s.formula == formula);
            let step = matching
                .next()
                .ok_or_else(|| err(format!("calibration {id} has no {formula} step")))?;
            if matching.next().is_some() {
                return Err(err(format!("calibration {id} has several {formula} steps")));
            }
            if step.values.len() != len {
                return Err(err(format!(
                    "calibration {id} {formula} step has {} values, expected {len}",
                    step.values.len()
                )));
            }
            Ok(step)
        };
        let traditional = find("Traditional", 2)?;
        let polynomial = find("Polynomial", POLYNOMIAL_VALUES)?;
        if let Some(other) = steps
            .iter()
            .find(|s| s.formula != "Traditional" && s.formula != "Polynomial")
        {
            return Err(err(format!(
                "calibration {id} has unsupported {} step",
                other.formula
            )));
        }
        let mut poly = [0.0; POLYNOMIAL_VALUES];
        poly.copy_from_slice(&polynomial.values);
        out.push(DefaultCalibration {
            id,
            traditional: [traditional.values[0], traditional.values[1]],
            polynomial_flags: polynomial.flags,
            polynomial: poly,
        });
    }
    if out.is_empty() {
        return Err(err("no DefaultCalibration entries"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(formula: &str, flags: u32, values: &[f64]) -> String {
        let mut s = format!(
            "<Step Number=\"1\"><CalibrationTechnique>ExternalReference</CalibrationTechnique>\
             <CalibrationFormula>{formula}</CalibrationFormula>\
             <NumberOfCoefficients>{}</NumberOfCoefficients>\
             <ValueUseFlags>{flags}</ValueUseFlags><Values>",
            values.len()
        );
        for (i, v) in values.iter().enumerate() {
            s.push_str(&format!("<Value Number=\"{}\">{v:E}</Value>", i + 1));
        }
        s.push_str("</Values></Step>");
        s
    }

    fn doc(cals: &[(i32, String)]) -> String {
        let mut s = String::from(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
             <DefaultMassCalibration><Version>1</Version><DefaultCalibrations>",
        );
        for (id, steps) in cals {
            s.push_str(&format!(
                "<DefaultCalibration DefaultCalibrationID=\"{id}\">{steps}</DefaultCalibration>"
            ));
        }
        s.push_str("</DefaultCalibrations></DefaultMassCalibration>");
        s
    }

    const POLY: [f64; 8] = [
        32245.5711007443,
        113125.231188769,
        1.94638932212931E-17,
        -1.75610630976578E-26,
        2.70160202926823E-31,
        -1.13530736865671E-36,
        0.0,
        0.0,
    ];

    fn valid() -> String {
        let steps = step("Traditional", 0, &[0.000347994641277165, 1018.75777725805])
            + &step("Polynomial", 232, &POLY);
        doc(&[(1, steps.clone()), (2, steps)])
    }

    #[test]
    fn parses_entries_with_bom() {
        let cals = parse_str(&valid()).unwrap();
        assert_eq!(cals.len(), 2);
        assert_eq!(cals[0].id, 1);
        assert_eq!(
            cals[0].traditional,
            [0.000347994641277165, 1018.75777725805]
        );
        assert_eq!(cals[0].polynomial_flags, 232);
        assert_eq!(cals[0].polynomial, POLY);
        let tof = cals[0].to_tof().unwrap();
        assert_eq!(tof.source, CalibrationSource::DefaultMassCal);
        assert!(((tof.mz(66165.7306301066) - 513.9653233122384) / 513.97).abs() < 1e-9);
    }

    #[test]
    fn missing_polynomial_step_is_an_error() {
        let steps = step("Traditional", 0, &[1.0, 2.0]);
        let err = parse_str(&doc(&[(1, steps)])).unwrap_err();
        assert!(err.to_string().contains("no Polynomial step"));
    }

    #[test]
    fn wrong_value_count_is_an_error() {
        let steps = step("Traditional", 0, &[1.0, 2.0]) + &step("Polynomial", 0, &[0.0; 6]);
        assert!(parse_str(&doc(&[(1, steps)])).is_err());
    }

    #[test]
    fn duplicate_id_and_empty_documents_are_errors() {
        let steps = step("Traditional", 0, &[1.0, 2.0]) + &step("Polynomial", 0, &[0.0; 8]);
        assert!(parse_str(&doc(&[(1, steps.clone()), (1, steps)])).is_err());
        assert!(parse_str(&doc(&[])).is_err());
        assert!(parse_str("not xml").is_err());
    }
}
