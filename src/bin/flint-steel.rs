//! Command-line runner for Flint JSON tests against in-memory Steel.

use std::env;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use flint_core::TestSpecLoadResult;
use flint_steel::{SteelAdapter, TestLoader, TestRunner};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("flint-steel: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let path = parse_path()?;
    flint_steel::init();

    let loader = TestLoader::unindexed(&path, true)
        .with_context(|| format!("could not discover tests at {}", path.display()))?;
    let paths = loader.collect_all_test_files()?;
    if paths.is_empty() {
        bail!("no JSON test files found at {}", path.display());
    }

    let specs = loader.load_specs(&paths, false)?;
    let compatible_count = specs
        .iter()
        .filter(|spec| matches!(spec, TestSpecLoadResult::Loaded(_)))
        .count();
    if compatible_count == 0 {
        bail!("no compatible tests loaded from {}", path.display());
    }
    if compatible_count != specs.len() {
        bail!(
            "{} of {} test(s) were skipped before execution",
            specs.len() - compatible_count,
            specs.len()
        );
    }

    let summary = TestRunner::new(Arc::new(SteelAdapter::new())).run_tests(&specs);
    serde_json::to_writer_pretty(io::stdout().lock(), &summary)?;
    io::stdout().write_all(b"\n")?;

    let executed = summary.total_tests.saturating_sub(summary.skipped_tests);
    if summary.total_tests != specs.len() {
        bail!(
            "runner returned {} results for {} loaded test records",
            summary.total_tests,
            specs.len()
        );
    }
    if executed == 0 {
        bail!("no tests executed");
    }
    if summary.skipped_tests != 0 {
        bail!("{} test(s) were skipped", summary.skipped_tests);
    }
    if summary.failed_tests != 0 {
        bail!("{} test(s) failed", summary.failed_tests);
    }
    Ok(())
}

fn parse_path() -> Result<PathBuf> {
    let mut args = env::args_os();
    let executable = args
        .next()
        .and_then(|path| PathBuf::from(path).file_name().map(ToOwned::to_owned))
        .unwrap_or_else(|| "flint-steel".into());
    let Some(path) = args.next() else {
        bail!(
            "usage: {} <test.json|test-directory>",
            PathBuf::from(executable).display()
        );
    };
    if args.next().is_some() {
        bail!("expected exactly one test file or directory");
    }
    let path = PathBuf::from(path);
    if !path.exists() {
        bail!("test path does not exist: {}", path.display());
    }
    Ok(path)
}
