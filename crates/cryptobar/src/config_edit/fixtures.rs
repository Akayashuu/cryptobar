use std::fs;

use super::Editor;

pub(super) fn editor() -> (tempfile::TempDir, Editor) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    (dir, Editor::new(path))
}

pub(super) fn seed(editor: &Editor, contents: &str) {
    fs::write(editor.path(), contents).unwrap();
}
