use tempfile::{NamedTempFile, TempDir};

fn create_temp_file() -> (TempDir, NamedTempFile) {
    let dir = TempDir::new().unwrap();
    let file = NamedTempFile::new_in(&dir).unwrap();
    (dir, file)
}
