#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

prose=$(git ls-files -z -- ':(exclude)Cargo.lock' ':(exclude)*.rs' ':(exclude)*.qml' \
    | xargs -0 grep -n -I -P '[\x{2013}\x{2014}]' /dev/null || true)
code_comments=$(git ls-files -z -- '*.rs' '*.qml' \
    | xargs -0 grep -n -I -P '^\s*//.*[\x{2013}\x{2014}]' /dev/null || true)
offenders=$(printf '%s\n%s\n' "$prose" "$code_comments" | sed '/^$/d')

if [ -n "$offenders" ]; then
    printf '%s\n' "$offenders" >&2
    echo "en dash (U+2013) and em dash (U+2014) are forbidden in prose and comments: use a colon, a comma, parentheses, a full stop or a hyphen" >&2
    exit 1
fi
