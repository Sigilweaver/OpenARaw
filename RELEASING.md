# Releasing OpenARaw

This repo ships a single crate/version pair: the `openaraw` Rust crate
(published to crates.io) and the `openaraw-py` Python bindings built from
it (published to PyPI as `openaraw`). Both crates share one version number
via `workspace.package.version` in the top-level `Cargo.toml`; the Python
package's version is `dynamic` and picked up from Cargo by maturin, so
there is no separate version to bump in `pyproject.toml`.

## Steps

1. **Update `CHANGELOG.md`.** Move the entries under `## [Unreleased]` to a
   new `## [X.Y.Z] - YYYY-MM-DD` section (today's date). Leave an empty
   `## [Unreleased]` heading at the top for the next round of PRs.

2. **Bump the version.** Edit `version` under `[workspace.package]` in the
   top-level `Cargo.toml`. This is the only version bump needed - both
   crates and the Python package inherit it.

3. **Commit and push to `main`.**

   ```
   git commit -am "release: vX.Y.Z"
   git push origin main
   ```

   (Follow the same PR flow you'd use for any other change if you'd
   rather not push directly to main.)

4. **Wait for CI and Audit to finish** on that commit, then confirm both
   are green:

   ```
   ./scripts/check-release-ready.sh
   ```

   This is the step that matters: `publish.yml` triggers on any `v*` tag
   push with no dependency on `ci.yml` or `audit.yml` passing (GitHub
   Actions can't express a `needs:` across separate workflow files), so
   nothing else stops a tag from kicking off a crates.io/PyPI publish off
   a red or unbuilt commit. Do not tag until this script exits 0.

   Note that `audit.yml` only runs on pushes that touch
   `Cargo.toml`/`Cargo.lock` (plus a weekly schedule) - a release commit
   always touches `Cargo.toml`, so it will normally trigger a fresh audit
   run on its own. If the script reports no audit run found, wait for it
   to finish rather than tagging against a stale or missing result.

5. **Tag the release commit** with an annotated tag:

   ```
   git tag -a vX.Y.Z -m "openaraw X.Y.Z - <short summary>"
   git push origin vX.Y.Z
   ```

   The tag push triggers `.github/workflows/publish.yml`, which publishes
   `openaraw` to crates.io and builds/publishes wheels + sdist to PyPI.

6. **Verify the publish.** Check that the new version shows up on
   [crates.io/crates/openaraw](https://crates.io/crates/openaraw) and
   [pypi.org/project/openaraw](https://pypi.org/project/openaraw/), and
   that the `Publish` workflow run completed successfully in the Actions
   tab.
