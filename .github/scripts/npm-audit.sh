#!/usr/bin/env bash
# `npm audit` at the severity CI fails on, tolerant of the registry.
#
# The audit endpoint is a network service. A 400 or 5xx from it exits 1 exactly
# like a real advisory would, so a bare `npm audit` turned main red once on an
# "Invalid package tree" registry error and skipped that commit's demo deploy.
# Distinguish the two by the JSON report: a completed audit carries
# `metadata.vulnerabilities`, a registry failure carries `error`. Retry the
# latter; fail on the former, printing the human report for the log.
set -uo pipefail

level=${NPM_AUDIT_LEVEL:-high}
attempts=${NPM_AUDIT_ATTEMPTS:-3}

for attempt in $(seq 1 "$attempts"); do
  report=$(npm audit --audit-level="$level" --json 2>/dev/null)
  status=$?
  if node -e '
    let doc;
    try { doc = JSON.parse(require("fs").readFileSync(0, "utf8")); } catch { process.exit(1); }
    process.exit(doc && doc.metadata && doc.metadata.vulnerabilities ? 0 : 1);
  ' <<<"$report"; then
    if [ "$status" -eq 0 ]; then
      echo "npm audit: no advisories at or above '$level'"
      exit 0
    fi
    echo "npm audit: advisories at or above '$level'"
    npm audit --audit-level="$level"
    exit 1
  fi
  echo "npm audit: no result from the registry (attempt $attempt of $attempts)"
  node -e 'try { const d = JSON.parse(require("fs").readFileSync(0, "utf8")); if (d.error) console.error(d.error.code || "", d.error.summary || d.error.detail || ""); } catch {}' <<<"$report"
  [ "$attempt" -lt "$attempts" ] && sleep 20
done

echo "::error::npm audit returned no result after $attempts attempts"
exit 1
