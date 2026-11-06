#!/usr/bin/env python3

import bisect
import sys

ALLOWED_LINE_PREFIXES = ("///", "//!")
RAW_PREFIXES = ("r", "br", "cr")
STRING_PREFIXES = ("b", "c")


def is_ident_start(char):
    return char.isalpha() or char == "_"


def is_ident_char(char):
    return char.isalnum() or char == "_"


def read_ident(source, index):
    while index < len(source) and is_ident_char(source[index]):
        index += 1
    return index


def line_comment_is_allowed(source, index):
    if source.startswith("////", index):
        return False
    return source.startswith(ALLOWED_LINE_PREFIXES, index)


def skip_line_comment(source, index):
    end = source.find("\n", index)
    return len(source) if end == -1 else end


def skip_block_comment(source, index):
    depth = 0
    cursor = index
    while cursor < len(source):
        if source.startswith("/*", cursor):
            depth += 1
            cursor += 2
        elif source.startswith("*/", cursor):
            depth -= 1
            cursor += 2
            if depth == 0:
                return cursor
        else:
            cursor += 1
    return len(source)


def skip_string(source, index):
    cursor = index
    while cursor < len(source):
        char = source[cursor]
        if char == "\\":
            cursor += 2
        elif char == '"':
            return cursor + 1
        else:
            cursor += 1
    return len(source)


def skip_raw_string(source, index, hashes):
    closing = '"' + "#" * hashes
    found = source.find(closing, index)
    return len(source) if found == -1 else found + len(closing)


def skip_quote(source, index):
    if source[index + 1 : index + 2] == "\\":
        cursor = index + 3
        while cursor < len(source) and source[cursor] not in "'\n":
            cursor += 1
        return cursor + 1 if source[cursor : cursor + 1] == "'" else cursor
    if source[index + 2 : index + 3] == "'":
        return index + 3
    if index + 1 < len(source) and is_ident_start(source[index + 1]):
        return read_ident(source, index + 1)
    return index + 1


def plain_comment_offsets(source):
    offsets = []
    cursor = 0
    while cursor < len(source):
        char = source[cursor]
        if source.startswith("//", cursor):
            if not line_comment_is_allowed(source, cursor):
                offsets.append(cursor)
            cursor = skip_line_comment(source, cursor)
        elif source.startswith("/*", cursor):
            offsets.append(cursor)
            cursor = skip_block_comment(source, cursor)
        elif char == '"':
            cursor = skip_string(source, cursor + 1)
        elif char == "'":
            cursor = skip_quote(source, cursor)
        elif is_ident_start(char):
            cursor = skip_identifier_or_literal(source, cursor)
        else:
            cursor += 1
    return offsets


def skip_identifier_or_literal(source, index):
    end = read_ident(source, index)
    word = source[index:end]
    follower = source[end : end + 1]
    if word in RAW_PREFIXES and follower in ("#", '"'):
        hashes = 0
        cursor = end
        while source[cursor : cursor + 1] == "#":
            hashes += 1
            cursor += 1
        if source[cursor : cursor + 1] == '"':
            return skip_raw_string(source, cursor + 1, hashes)
        return end
    if word in STRING_PREFIXES and follower == '"':
        return skip_string(source, end + 1)
    if word == "b" and follower == "'":
        return skip_quote(source, end)
    return end


def line_starts(source):
    return [offset for offset, char in enumerate(source) if char == "\n"]


def offenders_of(path, source):
    newlines = line_starts(source)
    report = []
    for offset in plain_comment_offsets(source):
        line_number = bisect.bisect_left(newlines, offset) + 1
        line_start = newlines[line_number - 2] + 1 if line_number > 1 else 0
        line_end = source.find("\n", offset)
        line_end = len(source) if line_end == -1 else line_end
        report.append(f"{path}:{line_number}: {source[line_start:line_end].strip()}")
    return report


SELF_TEST_FLAGGED = [
    ('let p = r"C:\\"; // real', 1),
    ('let s = "a\n/// not a doc\n";\nlet t = 1; // c', 1),
    ("let x = 1; //// hmm", 1),
    ("/* outer /* inner */ still */ fn f() {}", 1),
    ("/** doc */ fn f() {}", 1),
    ("/*! inner */ fn f() {}", 1),
    ("let x = 1; // plain", 1),
    ("let c = '\"'; // after", 1),
    ("let c = '\\''; // after", 1),
    ("let c = '\\u{1F600}'; // after", 1),
    ('let s = "a \\" // not"; // real', 1),
    ('let s = b"x"; // real', 1),
    ('let s = br#"x"#; // real', 1),
    ('let s = c"x"; // real', 1),
    ('let s = cr"x"; // real', 1),
    ("let v = b'\\''; // real", 1),
    ("fn f<'a>(x: &'a str) -> &'static str { x } // c", 1),
    ("fn f() { 'outer: loop { break 'outer; } } // c", 1),
    ('let s = "http://x"; // real', 1),
    ("/* a\nb */", 1),
]

SELF_TEST_CLEAN = [
    ("/// doc\n//! inner\nfn f() {}", 0),
    ('let p = r"C:\\";', 0),
    ('let s = r#"a " // b"#;', 0),
    ('let s = r###"x "## // y "###;', 0),
    ('let s = "http://example.com";', 0),
    ('let s = "a /* b";', 0),
    ("let c = '\"'; let d = 'x';", 0),
    ("let c = '\\'';", 0),
    ("let u = '\\u{1F600}';", 0),
    ("fn f<'a>(x: &'a str) -> &'static str { x }", 0),
    ("fn f() { 'outer: loop { break 'outer; } }", 0),
    ("let r#type = 1;", 0),
    ('let s = "multi\n/// line\n";', 0),
    ('let s = b"\\\\";', 0),
    ("let x = 1 / 2; let y = x /2;", 0),
    ("/// a */ b", 0),
    ("//! crate doc with \"quote and // inside", 0),
    ("/// see `'a`", 0),
]


def self_test():
    failures = []
    cases = [(source, count, True) for source, count in SELF_TEST_FLAGGED]
    cases += [(source, count, False) for source, count in SELF_TEST_CLEAN]
    for source, expected, _flagged in cases:
        found = len(plain_comment_offsets(source))
        if found != expected:
            failures.append(f"expected {expected}, found {found}: {source!r}")
    if failures:
        print("comment lexer self-test failed:", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"comment lexer self-test: {len(cases)} cases ok")
    return 0


def main(arguments):
    if arguments == ["--self-test"]:
        return self_test()
    offenders = []
    for path in arguments:
        with open(path, encoding="utf-8") as source_file:
            offenders.extend(offenders_of(path, source_file.read()))
    if offenders:
        print("\n".join(offenders), file=sys.stderr)
        print(
            "plain // and /* */ comments are forbidden in Rust code: use rustdoc (/// or //!) on public items",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
