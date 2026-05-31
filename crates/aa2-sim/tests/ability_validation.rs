/// Integration test: validates all RON data files in the repo's data/ directory.
///
/// Ensures every ability definition passes structural validation and can be
/// smoke-resolved at all levels without panicking.
#[test]
fn all_data_files_are_valid() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data_dir = manifest_dir.join("../../data");
    let errors = aa2_sim::validate_data_dir(&data_dir);
    assert!(
        errors.is_empty(),
        "Data validation errors:\n{}",
        errors.join("\n")
    );
}
