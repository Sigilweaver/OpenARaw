"""Real-file smoke test for the Agilent Python field surface."""

from __future__ import annotations

import os
from pathlib import Path

import openaraw
import pytest


def test_decoded_fields():
    path = Path(os.environ.get("OPENARAW_TEST_BUNDLE", "corpus/180814-Sample19.d"))
    if not path.is_dir():
        if os.environ.get("REQUIRE_CORPUS") == "1":
            pytest.fail(f"REQUIRE_CORPUS=1 but bundle {path} is missing")
        pytest.skip("set OPENARAW_TEST_BUNDLE to an Agilent bundle")
    reader = openaraw.RawReader(str(path))
    run = reader.run_info()
    assert run["source_file_name"] == path.name
    assert "openaraw.msscan_stride" in run["extra"]
    assert reader.device_info() is None or reader.device_info()["model"]
    index = reader.scan_index()
    assert len(index["records"]) == reader.scan_count
    sources = {r["calibration_source"] for r in index["records"]}
    assert sources <= {"mass_cal_record", "default_mass_cal", None}
    if index["stride"] in (216, 220, 284):
        # Q-TOF: every scan has a calibration, so m/z is never a flight time.
        assert None not in sources
        mz = next(s for s in openaraw.iter_spectra(str(path)) if len(s.mz)).mz
        assert 10 < min(mz) and max(mz) < 20000
    first = reader.read_spectrum(0)
    streamed = next(openaraw.iter_spectra(str(path)))
    assert first.native_id == streamed.native_id
    # IonPolarity 0 is positive, 1 negative; other values leave it None.
    raw = index["records"][0]["ion_polarity"]
    assert first.polarity == {0: "positive", 1: "negative"}.get(raw)
    assert len(first.mz) == len(first.intensity)
    for chrom in reader.read_chromatograms():
        assert len(chrom["time_sec"]) == len(chrom["intensity"])


def _bundle(env: str, default: str) -> Path:
    path = Path(os.environ.get(env, default))
    if not path.is_dir():
        if os.environ.get("REQUIRE_CORPUS") == "1":
            pytest.fail(f"REQUIRE_CORPUS=1 but bundle {path} is missing")
        pytest.skip(f"set {env} to an Agilent bundle")
    return path


def test_qtof_ms2_record_keys():
    """MS/MS records carry collision_energies (value and unit), no unit assumed."""
    path = _bundle("OPENARAW_TEST_BUNDLE", "corpus/180814-Sample19.d")
    reader = openaraw.RawReader(str(path))
    ms2 = [
        reader.read_spectrum(i)
        for i in range(reader.scan_count)
        if reader.read_spectrum(i).ms_level == 2
    ]
    assert ms2, "the fixture has MS/MS spectra"
    spectrum = ms2[0]
    precursor = spectrum.precursor
    assert "collision_energy" not in precursor
    assert "ce_is_nce" not in precursor
    energies = precursor["collision_energies"]
    assert len(energies) == 1
    assert set(energies[0]) == {"value", "unit"}
    assert energies[0]["value"] > 0
    # The file states no unit for the scan record's CollisionEnergy field.
    assert energies[0]["unit"] is None
    # The value is the scan record's, unchanged.
    index = reader.scan_index()["records"]
    raw = [r for r in index if r["ms_level"] == 2][0]
    assert energies[0]["value"] == raw["collision_energy"]
    assert precursor["target_mz"] == raw["target_mz"]
    assert isinstance(spectrum.retention_time_sec, float)
    assert spectrum.analyzer == "tof"


def test_mrm_record_keys():
    """MRM precursors carry no spectrum reference and no collision energy."""
    path = _bundle("OPENARAW_TEST_MRM_BUNDLE", "corpus/Cdc19_ubp2_AQUA.d")
    reader = openaraw.RawReader(str(path))
    spectrum = reader.read_spectrum(0)
    assert spectrum.ms_level == 2
    assert spectrum.analyzer == "tqms"
    precursor = spectrum.precursor
    assert precursor["precursor_native_id"] is None
    assert precursor["collision_energies"] == []
    assert "openaraw.mrm_channel_id" in spectrum.extra


def test_undecodable_scans_raise():
    # PXD022473/M.d: scan records point into an MSProfile.bin the upload
    # lacks, so no scan can be decoded.
    root = os.environ.get("OPENARAW_CORPUS")
    if not root:
        pytest.skip("set OPENARAW_CORPUS to the research corpus root")
    path = Path(root) / "PXD022473" / "M.d"
    if not path.is_dir():
        if os.environ.get("REQUIRE_CORPUS") == "1":
            pytest.fail(f"REQUIRE_CORPUS=1 but bundle {path} is missing")
        pytest.skip(f"{path} is missing")
    with pytest.raises(RuntimeError, match="profile block"):
        openaraw.RawReader(str(path))
    with pytest.raises(RuntimeError, match="profile block"):
        next(openaraw.iter_spectra(str(path)))
