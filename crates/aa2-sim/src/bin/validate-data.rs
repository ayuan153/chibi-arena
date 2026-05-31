//! Author-facing data validation tool.
//!
//! Validates all RON game data files and prints a readable report.
//! Exits 0 if all data is valid, 1 otherwise.

fn main() {
    let data_dir = std::env::args().nth(1).unwrap_or_else(|| {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{manifest_dir}/../../data")
    });
    let dir = std::path::Path::new(&data_dir);

    let errors = aa2_sim::validate_data_dir(dir);
    if errors.is_empty() {
        println!("✓ All data files valid");
    } else {
        eprintln!("✗ {} validation error(s):", errors.len());
        for e in &errors {
            eprintln!("  - {e}");
        }
        std::process::exit(1);
    }
}
