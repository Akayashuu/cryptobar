#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

git ls-files -z -- '*.rs' | xargs -0 -r python3 -I scripts/check-no-comments.py
