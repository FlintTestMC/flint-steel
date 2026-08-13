//! Red controls that prove the Steel adapter observes real failures.

use std::path::PathBuf;
use std::sync::Arc;

use flint_core::{TestLoader, TestRunner};
use flint_steel::SteelAdapter;

#[test]
fn checked_in_negative_controls_turn_red() {
    flint_steel::init();
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mvp");
    let paths = [
        "cave_air_distinct_negative.json",
        "invalid_block_negative.json",
        "wrong_block_negative.json",
    ]
    .map(|name| fixture_root.join(name));
    let loader = TestLoader::unindexed(&fixture_root, true).expect("fixture directory should load");
    let specs = loader
        .load_specs(&paths, false)
        .expect("negative fixtures should parse");
    let summary = TestRunner::new(Arc::new(SteelAdapter::new())).run_tests(&specs);

    assert_eq!(summary.total_tests, 3, "all negative controls must load");
    assert_eq!(summary.skipped_tests, 0, "negative controls must execute");
    assert_eq!(
        summary.failed_tests, 3,
        "all negative controls must turn red"
    );

    let cave_air = summary
        .results
        .iter()
        .find(|result| result.test_name == "cave_air_distinct_negative")
        .expect("missing cave-air result");
    assert_eq!(
        cave_air.failed_count(),
        1,
        "cave air must remain distinct from regular air"
    );

    let invalid = summary
        .results
        .iter()
        .find(|result| result.test_name == "invalid_block_negative")
        .expect("missing invalid-block result");
    assert!(
        invalid
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("unknown block")),
        "invalid block failed for the wrong reason: {invalid:?}"
    );

    let wrong = summary
        .results
        .iter()
        .find(|result| result.test_name == "wrong_block_negative")
        .expect("missing wrong-block result");
    assert_eq!(wrong.failed_count(), 1, "wrong assertion must be observed");
}
