#!/usr/bin/env bash
# Confirms CI and the dependency audit are green for a commit before it gets
# tagged for release. GitHub Actions has no way for publish.yml to `needs:` a
# job defined in ci.yml/audit.yml (separate workflow files can't depend on
# each other), so this has to be enforced by hand before tagging instead of
# inside publish.yml itself. See RELEASING.md and issue #8.
#
# Usage: scripts/check-release-ready.sh [ref]
#   ref defaults to HEAD.

set -euo pipefail

REF="${1:-HEAD}"

SHA="$(git rev-parse "$REF")"
echo "Checking release readiness for $REF ($SHA)"
echo

overall_ok=1

check_workflow() {
    local workflow_file="$1"
    local label="$2"

    local run_json
    run_json="$(gh run list -w "$workflow_file" -c "$SHA" --json status,conclusion,url -L 1)"

    if [[ "$run_json" == "[]" ]]; then
        echo "FAIL: no $label run found for commit $SHA (workflow: $workflow_file)."
        overall_ok=0
        return
    fi

    local status conclusion url
    status="$(jq -r '.[0].status' <<<"$run_json")"
    conclusion="$(jq -r '.[0].conclusion' <<<"$run_json")"
    url="$(jq -r '.[0].url' <<<"$run_json")"

    if [[ "$status" != "completed" ]]; then
        echo "FAIL: latest $label run for $SHA is not finished yet (status: $status)."
        echo "      $url"
        overall_ok=0
        return
    fi

    if [[ "$conclusion" != "success" ]]; then
        echo "FAIL: latest $label run for $SHA did not succeed (conclusion: $conclusion)."
        echo "      $url"
        overall_ok=0
        return
    fi

    echo "OK: $label passed for $SHA."
    echo "    $url"
}

check_workflow "ci.yml" "CI"
check_workflow "audit.yml" "Audit"

echo

if [[ "$overall_ok" -eq 1 ]]; then
    echo "Release ready: CI and Audit are both green for $SHA."
    exit 0
else
    echo "Not release ready: fix the failures above before tagging $SHA."
    exit 1
fi
