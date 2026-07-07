# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| latest  | Yes       |
| older   | No        |

Only the latest published release receives security updates.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report privately via [GitHub Security Advisories](https://github.com/Sigilweaver/OpenARaw/security/advisories/new).

Include:

- A description of the vulnerability and its potential impact.
- Steps to reproduce or a proof of concept (a small `.d` directory is
  ideal; small synthetic byte sequences for individual `.bin` files are
  even better).
- The crate version (Rust or Python wheel) and OS / toolchain.

Expect an initial acknowledgment within 7 days.

## Scope

In scope:

- **Parser correctness on malicious `.d` input.** OpenARaw parses several
  binary files inside an Agilent `.d` directory (`MSScan.bin`,
  `MSPeak.bin`, `MSProfile.bin`, `MSPeriodicActuals.bin`,
  `MSMassCal.bin`) plus the XML files that accompany them. Panics,
  out-of-bounds reads, undefined behavior, infinite loops, or memory
  exhaustion triggered by a crafted or truncated `.bin` file, or by
  malformed/adversarial XML, are in scope.
- **Memory safety**: the `openaraw` crate forbids `unsafe_code`. A
  demonstrated unsafe-code violation reachable from safe API is a
  security bug. (`openaraw-py` allows `unsafe_code` for PyO3's generated
  bindings; a memory-safety issue there is still in scope.)
- **Path-traversal or arbitrary-file-write bugs** in any helper that
  derives output paths from input filenames or from paths found inside a
  `.d` directory.
- **Supply-chain integrity** of published artifacts on crates.io and
  PyPI.

Out of scope:

- Denial of service via legitimately large `.d` directories. Real
  acquisitions can be hundreds of GB by design.
- Inaccurate decoding of specific Agilent acquisition modes. Those are
  correctness bugs - file them as regular issues.
- Vulnerabilities in third-party crates with no demonstrated exploit path
  through OpenARaw.

## Disclosure

We follow coordinated disclosure. Reporters are credited in the release
notes unless they prefer to remain anonymous. We aim to ship a fix within
30 days of confirming a high or critical issue.

## Note on reverse engineering

OpenARaw was developed by clean-room reverse engineering of public
artifacts (PRIDE deposits, the XML/XSD schema files Agilent ships
alongside its own data, published specifications). It does not depend on
any Agilent SDK or binary blob, and contains no Agilent proprietary code.
Bug reports about parser accuracy or coverage are welcome but are not
security issues unless they involve one of the categories above.

## Stack context

OpenARaw is one of several vendor readers in the
[OpenProteo](https://github.com/Sigilweaver/OpenProteo) stack. Sibling
readers: [OpenTFRaw](https://github.com/Sigilweaver/OpenTFRaw) (Thermo),
[OpenWRaw](https://github.com/Sigilweaver/OpenWRaw) (Waters),
[OpenTimsTDF](https://github.com/Sigilweaver/OpenTimsTDF) (Bruker).
Shared foundation:
[openproteo-core](https://github.com/Sigilweaver/OpenProteoCore).
