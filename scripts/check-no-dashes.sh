#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

export LC_ALL=C.UTF-8

dash='[\x{2013}\x{2014}]'
offenders=0

scan() {
    local label=$1 pattern=$2
    shift 2
    local hits rc=0
    hits=$(git grep -I -n -P "$pattern" -- "$@") || rc=$?
    case $rc in
        0)
            printf '%s\n' "$hits" >&2
            offenders=1
            ;;
        1) ;;
        *)
            echo "dash gate failed on $label (git grep exit $rc)" >&2
            exit 2
            ;;
    esac
}

scan prose "$dash" . ':(exclude)Cargo.lock' ':(exclude)*.rs' ':(exclude)*.qml'
scan code-comments "^\\s*//.*$dash" '*.rs' '*.qml'

if (( offenders )); then
    echo "en dash (U+2013) and em dash (U+2014) are forbidden in prose and comments: use a colon, a comma, parentheses, a full stop or a hyphen" >&2
    exit 1
fi
