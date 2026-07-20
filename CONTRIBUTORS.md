# Contributors

Thank you to everyone who has contributed to OpenARaw.

## Benjamin Riley ([@Nabejo](https://github.com/Nabejo))

Contributed in v0.1.3:

- **Allocation caps in the `.d` decoders** - `read_bytes` now checks a
  requested `ByteCount` against the file's actual remaining size before
  allocating, and `lzf::decompress` caps its `UncompressedByteCount`-driven
  allocation at 512 MiB, closing off multi-GB allocations from crafted
  files.
- **Decoder unit tests** - synthetic byte-slice tests for the `lzf`,
  `msscan`, `mspeak`, and `msprofile` block decoders, previously only
  covered by corpus-gated conformance tests.
