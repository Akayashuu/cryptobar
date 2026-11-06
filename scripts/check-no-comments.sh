#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

offenders=$(git ls-files -z -- '*.rs' | xargs -0 perl -ne '
    $in_string = 0 if $. == 1;
    if (!m{^\s*//[/!]}) {
        my $code = "";
        my $i = 0;
        while ($i < length $_) {
            my $c = substr($_, $i, 1);
            my $rest = substr($_, $i);
            if ($in_string) {
                if ($c eq "\\") { $i += 2; next }
                $in_string = 0 if $c eq "\"";
                $i++;
                next;
            }
            if ($rest =~ m{\A(\x27(?:\\.|[^\\\x27])\x27)}) { $code .= $1; $i += length $1; next }
            if ($rest =~ m{\A//}) { $code .= $rest; last }
            if ($c eq "\"") { $in_string = 1; $i++; next }
            $code .= $c;
            $i++;
        }
        $code =~ s{\w+://\S*}{}g;
        print "$ARGV:$.: $_" if $code =~ m{(?:^|[^/])//(?:[^/!]|$)|^////|/\*};
    }
    close ARGV if eof;
')

if [ -n "$offenders" ]; then
    printf '%s\n' "$offenders" >&2
    echo "plain // and /* */ comments are forbidden in Rust code: use rustdoc (/// or //!) on public items" >&2
    exit 1
fi
