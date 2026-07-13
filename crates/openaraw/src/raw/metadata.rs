//! Run-level metadata parsed from the small XML sidecar files that ship
//! alongside the binary payload in an Agilent `.d` bundle:
//!
//! - `AcqData/Devices.xml` lists every hardware device attached to the
//!   acquisition (mass spectrometer, pumps, autosampler, column oven,
//!   ...). The mass spectrometer is always the first `<Device>` element
//!   in document order - confirmed across 330 real-world bundles pulled
//!   from PRIDE (see `CORPUS.md`), where it is always `Name=QTOF` /
//!   `Type=6` or `Name=TandemQuadrupole` / `Type=5`, both with
//!   `StoredDataType=8`. Its `<ModelNumber>` (e.g. `G6550A`) is the real,
//!   vendor-assigned instrument identifier - unlike the previous
//!   `MSScan.bin` record-stride guess, which only distinguished "wide
//!   record" from "narrow record" and mapped that to a hardcoded string.
//! - `AcqData/Contents.xml` carries a single run-level `<AcquiredTime>`
//!   element with the acquisition start time. Observed corpus values are
//!   already RFC 3339 (either `2014-08-06T22:49:56Z` or an offset form
//!   with fractional seconds, e.g. `2021-11-30T18:36:39.6755756-07:00`),
//!   so the value is passed through as-is rather than reformatted.
//!
//! Both files are best-effort: a handful of real-world bundles are
//! missing one or both (see `docs/format/06-known-limitations.md`), so
//! every function here returns `None` on any I/O or structural problem
//! instead of failing bundle open.

use std::path::Path;

/// The mass spectrometer entry read from `Devices.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// `<Name>`, e.g. `"QTOF"` or `"TandemQuadrupole"`.
    pub name: String,
    /// `<ModelNumber>`, e.g. `"G6550A"` or `"G6410A"`.
    pub model: String,
}

/// Read `Devices.xml` and return the first `<Device>` element, which is
/// always the mass spectrometer itself (see module docs). Returns `None`
/// if the file is absent, unparseable, or has no `<Device>` children with
/// both `<Name>` and `<ModelNumber>` present.
pub fn parse_devices_xml(path: &Path) -> Option<DeviceInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let doc = roxmltree::Document::parse(&text).ok()?;
    let device = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Device")?;

    let name = child_text(device, "Name")?;
    let model = child_text(device, "ModelNumber")?;
    if name.is_empty() || model.is_empty() {
        return None;
    }
    Some(DeviceInfo { name, model })
}

/// Read `Contents.xml` and return the trimmed `<AcquiredTime>` text.
/// Returns `None` if the file is absent, unparseable, or the element is
/// missing/empty.
pub fn parse_acquired_time(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let doc = roxmltree::Document::parse(&text).ok()?;
    let root = doc.root_element();
    let value = child_text(root, "AcquiredTime")?;
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn child_text(node: roxmltree::Node, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "openaraw-metadata-test-{}-{}",
            std::process::id(),
            name
        ));
        let mut f = std::fs::File::create(&path).expect("create temp file");
        f.write_all(contents.as_bytes()).expect("write temp file");
        path
    }

    #[test]
    fn parses_first_device_as_qtof() {
        let path = write_temp(
            "devices-qtof.xml",
            r#"<?xml version="1.0" encoding="utf-8"?>
<Devices>
  <Version>1</Version>
  <Device DeviceID="1">
    <Name>QTOF</Name>
    <ModelNumber>G6550A</ModelNumber>
    <Type>6</Type>
  </Device>
  <Device DeviceID="2">
    <Name>LowflowPump</Name>
    <ModelNumber>G1376A</ModelNumber>
    <Type>35</Type>
  </Device>
</Devices>"#,
        );
        let info = parse_devices_xml(&path).expect("should parse");
        assert_eq!(info.name, "QTOF");
        assert_eq!(info.model, "G6550A");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parses_first_device_as_qqq() {
        let path = write_temp(
            "devices-qqq.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Devices>
  <Device DeviceID="1">
    <Name>TandemQuadrupole</Name>
    <ModelNumber>G6410A</ModelNumber>
    <Type>5</Type>
  </Device>
</Devices>"#,
        );
        let info = parse_devices_xml(&path).expect("should parse");
        assert_eq!(info.name, "TandemQuadrupole");
        assert_eq!(info.model, "G6410A");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_devices_file_returns_none() {
        let path = Path::new("/nonexistent/Devices.xml");
        assert_eq!(parse_devices_xml(path), None);
    }

    #[test]
    fn parses_acquired_time_with_z_suffix() {
        let path = write_temp(
            "contents-z.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Contents>
  <Version>3</Version>
  <AcquiredTime>2014-08-06T22:49:56Z</AcquiredTime>
</Contents>"#,
        );
        assert_eq!(
            parse_acquired_time(&path).as_deref(),
            Some("2014-08-06T22:49:56Z")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parses_acquired_time_with_offset_and_fractional_seconds() {
        let path = write_temp(
            "contents-offset.xml",
            r#"<?xml version="1.0"?>
<Contents>
  <AcquiredTime>2021-11-30T18:36:39.6755756-07:00</AcquiredTime>
</Contents>"#,
        );
        assert_eq!(
            parse_acquired_time(&path).as_deref(),
            Some("2021-11-30T18:36:39.6755756-07:00")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_contents_file_returns_none() {
        let path = Path::new("/nonexistent/Contents.xml");
        assert_eq!(parse_acquired_time(path), None);
    }

    #[test]
    fn empty_acquired_time_returns_none() {
        let path = write_temp(
            "contents-empty.xml",
            r#"<Contents><AcquiredTime></AcquiredTime></Contents>"#,
        );
        assert_eq!(parse_acquired_time(&path), None);
        let _ = std::fs::remove_file(&path);
    }
}
