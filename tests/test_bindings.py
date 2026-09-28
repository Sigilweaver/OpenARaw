"""Real-file smoke test for the Agilent Python field surface."""

from __future__ import annotations

import os
from pathlib import Path

import openaraw
import pytest


def test_decoded_fields():
    path = Path(os.environ.get("OPENARAW_TEST_BUNDLE", "corpus/180814-Sample19.d"))
    if not path.is_dir():
        pytest.skip("set OPENARAW_TEST_BUNDLE to an Agilent bundle")
    reader = openaraw.RawReader(str(path))
    run = reader.run_info()
    assert run["source_file_name"] == path.name
    assert "openaraw.msscan_stride" in run["extra"]
    assert reader.device_info() is None or reader.device_info()["model"]
    index = reader.scan_index()
    assert len(index["records"]) == reader.scan_count
    first = reader.read_spectrum(0)
    streamed = next(openaraw.iter_spectra(str(path)))
    assert first.native_id == streamed.native_id
    assert len(first.mz) == len(first.intensity)
    for chrom in reader.read_chromatograms():
        assert len(chrom["time_sec"]) == len(chrom["intensity"])
