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
    assert len(first.mz) == len(first.intensity)
    for chrom in reader.read_chromatograms():
        assert len(chrom["time_sec"]) == len(chrom["intensity"])


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
