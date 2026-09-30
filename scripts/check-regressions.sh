#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

ruby scripts/check-regressions-core.rb
ruby scripts/check-regressions-workloads.rb
ruby scripts/check-regressions-resources.rb
ruby scripts/check-regressions-tooling.rb
