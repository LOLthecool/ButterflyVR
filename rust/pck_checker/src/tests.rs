#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    #[test]
    fn test_clean() {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test_data")
            .join("clean_examples");

        let entries: Vec<_> = fs::read_dir(fixture_dir)
            .expect("Failed to read test data directory")
            .map(|entry| entry.expect("Failed to read directory entry").path())
            .filter(|path| path.is_file())
            .collect();

        for path in entries {
            println!("Testing: {}", path.display());
            assert!(
                crate::PCKChecker::is_pck_good(
                    path.clone().into_string().unwrap(),
                    "0".to_string(),
                    "00000000-0000-0000-0000-000000000000".to_string()
                ),
                "failed good pck {:?}",
                path
            );
        }
    }

    #[test]
    fn test_evil() {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test_data")
            .join("evil_examples");

        let entries: Vec<_> = fs::read_dir(fixture_dir)
            .expect("Failed to read test data directory")
            .map(|entry| entry.expect("Failed to read directory entry").path())
            .filter(|path| path.is_file())
            .collect();

        for path in entries {
            println!("Testing: {}", path.display());
            assert!(
                !crate::PCKChecker::is_pck_good(
                    path.clone().into_string().unwrap(),
                    "0".to_string(),
                    "00000000-0000-0000-0000-000000000000".to_string()
                ),
                "failed good pck {:?}",
                path
            );
        }
    }
}
