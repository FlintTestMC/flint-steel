//! Green controls that exercise the complete Steel adapter path.

use std::path::PathBuf;
use std::sync::Arc;

use flint_core::{TestLoader, TestRunner};
use flint_steel::SteelAdapter;

#[test]
fn checked_in_positive_controls_all_turn_green() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mvp");
    let paths = [
        "block_fill_positive.json",
        "inventory_positive.json",
        "place_fence_positive.json",
        "time_advance_positive.json",
    ]
    .map(|name| fixture_root.join(name));
    let loader = TestLoader::unindexed(&fixture_root, true).expect("fixture directory should load");
    let specs = loader
        .load_specs(&paths, false)
        .expect("positive fixtures should parse");
    let summary = TestRunner::new(Arc::new(SteelAdapter::new())).run_tests(&specs);

    assert_eq!(summary.total_tests, 4, "all positive controls must load");
    assert_eq!(summary.skipped_tests, 0, "positive controls must execute");
    assert_eq!(
        summary.failed_tests, 0,
        "positive controls failed: {:#?}",
        summary.results
    );
    assert_eq!(summary.passed_tests, 4, "all positive controls must pass");
}
