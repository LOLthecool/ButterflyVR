#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    #[test]
    fn test_clean() {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test_data")
            .join("clean_examples");

        let mut entries: Vec<_> = fs::read_dir(fixture_dir)
            .expect("Failed to read test data directory")
            .map(|entry| entry.expect("Failed to read directory entry").path())
            .filter(|path| path.is_file())
            .collect();

        for path in &entries {
            println!("Testing: {}", path.display());
            crate::
        }
    }

    #[test]
    fn test_evil() {}
}
