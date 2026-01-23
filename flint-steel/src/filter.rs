//! Test filtering and selection.
//!
//! Provides ways to select which tests to run based on tags, names, or patterns.

use std::path::Path;

use anyhow::Result;
use flint_core::loader::TestLoader;
use flint_core::test_spec::TestSpec;

/// Criteria for selecting tests to run.
#[derive(Debug, Clone, Default)]
pub struct TestFilter {
    /// Run only tests with these tags (empty = no tag filter)
    pub tags: Vec<String>,
    /// Run only tests matching these name patterns (supports glob: `*`, `?`)
    pub name_patterns: Vec<String>,
    /// Run only this specific test by exact name
    pub exact_name: Option<String>,
}

impl TestFilter {
    /// Create a filter that matches all tests.
    pub fn all() -> Self {
        Self::default()
    }

    /// Create a filter for tests with specific tags.
    pub fn by_tags(tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            tags: tags.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    /// Create a filter for a single test by exact name.
    pub fn by_name(name: impl Into<String>) -> Self {
        Self {
            exact_name: Some(name.into()),
            ..Default::default()
        }
    }

    /// Create a filter for tests matching name patterns (glob syntax).
    pub fn by_patterns(patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            name_patterns: patterns.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    /// Add tags to the filter (builder pattern).
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Add name patterns to the filter (builder pattern).
    pub fn with_patterns(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.name_patterns.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Set exact name match (builder pattern).
    pub fn with_exact_name(mut self, name: impl Into<String>) -> Self {
        self.exact_name = Some(name.into());
        self
    }

    /// Check if a test spec matches this filter.
    pub fn matches(&self, spec: &TestSpec) -> bool {
        // Check exact name first
        if let Some(ref exact) = self.exact_name {
            if spec.name != *exact {
                return false;
            }
        }

        // Check name patterns
        if !self.name_patterns.is_empty() {
            let matches_any = self.name_patterns.iter().any(|pattern| {
                glob_match(pattern, &spec.name)
            });
            if !matches_any {
                return false;
            }
        }

        // Check tags (test must have at least one matching tag)
        if !self.tags.is_empty() {
            let has_matching_tag = self.tags.iter().any(|tag| {
                spec.tags.iter().any(|t| t == tag)
            });
            if !has_matching_tag {
                return false;
            }
        }

        true
    }

    /// Returns true if no filters are set (matches everything).
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.name_patterns.is_empty() && self.exact_name.is_none()
    }
}

/// Simple glob matching supporting `*` (any chars) and `?` (single char).
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    glob_match_recursive(&pattern_chars, &text_chars, 0, 0)
}

fn glob_match_recursive(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
    if pi == pattern.len() && ti == text.len() {
        return true;
    }
    if pi == pattern.len() {
        return false;
    }

    match pattern[pi] {
        '*' => {
            // Try matching zero or more characters
            for i in ti..=text.len() {
                if glob_match_recursive(pattern, text, pi + 1, i) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // Match exactly one character
            if ti < text.len() {
                glob_match_recursive(pattern, text, pi + 1, ti + 1)
            } else {
                false
            }
        }
        c => {
            // Match literal character
            if ti < text.len() && text[ti] == c {
                glob_match_recursive(pattern, text, pi + 1, ti + 1)
            } else {
                false
            }
        }
    }
}

/// Load and filter tests from a directory.
pub struct TestSelector {
    loader: TestLoader,
}

impl TestSelector {
    /// Create a new test selector for the given test directory.
    pub fn new(test_path: &Path) -> Result<Self> {
        let loader = TestLoader::new(test_path, true)?;
        Ok(Self { loader })
    }

    /// Load all test specs, optionally filtered.
    pub fn load_tests(&self, filter: &TestFilter) -> Result<Vec<TestSpec>> {
        // Get paths based on tag filter (if any)
        let paths = if !filter.tags.is_empty() {
            self.loader.collect_by_tags(&filter.tags)?
        } else {
            self.loader.collect_all_test_files()?
        };

        // Load specs and apply additional filters
        let mut specs = Vec::new();
        for path in paths {
            match TestSpec::from_file(&path) {
                Ok(spec) => {
                    if filter.matches(&spec) {
                        specs.push(spec);
                    }
                }
                Err(e) => {
                    // Log but don't fail - continue with other tests
                    eprintln!("Warning: Failed to load test {}: {}", path.display(), e);
                }
            }
        }

        Ok(specs)
    }

    /// Load a single test by exact name.
    pub fn load_test_by_name(&self, name: &str) -> Result<Option<TestSpec>> {
        let paths = self.loader.collect_all_test_files()?;

        for path in paths {
            if let Ok(spec) = TestSpec::from_file(&path) {
                if spec.name == name {
                    return Ok(Some(spec));
                }
            }
        }

        Ok(None)
    }

    /// Get all available test names.
    pub fn list_test_names(&self) -> Result<Vec<String>> {
        let paths = self.loader.collect_all_test_files()?;
        let mut names = Vec::new();

        for path in paths {
            if let Ok(spec) = TestSpec::from_file(&path) {
                names.push(spec.name);
            }
        }

        names.sort();
        Ok(names)
    }

    /// Get all available tags.
    pub fn list_tags(&self) -> Result<Vec<String>> {
        let paths = self.loader.collect_all_test_files()?;
        let mut tags = std::collections::HashSet::new();

        for path in paths {
            if let Ok(spec) = TestSpec::from_file(&path) {
                for tag in spec.tags {
                    tags.insert(tag);
                }
            }
        }

        let mut tags: Vec<_> = tags.into_iter().collect();
        tags.sort();
        Ok(tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spec(name: &str, tags: &[&str]) -> TestSpec {
        TestSpec {
            flint_version: Some("0.1".to_string()),
            name: name.to_string(),
            description: None,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            dependencies: vec![],
            setup: None,
            timeline: vec![],
            breakpoints: vec![],
        }
    }

    // ==========================================================================
    // TestFilter Tests
    // ==========================================================================

    #[test]
    fn test_filter_all_matches_everything() {
        let filter = TestFilter::all();
        let spec = make_spec("any_test", &["tag1", "tag2"]);
        assert!(filter.matches(&spec));
    }

    #[test]
    fn test_filter_is_empty() {
        assert!(TestFilter::all().is_empty());
        assert!(!TestFilter::by_tags(["foo"]).is_empty());
        assert!(!TestFilter::by_name("test").is_empty());
        assert!(!TestFilter::by_patterns(["*"]).is_empty());
    }

    #[test]
    fn test_filter_by_exact_name() {
        let filter = TestFilter::by_name("copper_waxing");

        assert!(filter.matches(&make_spec("copper_waxing", &[])));
        assert!(!filter.matches(&make_spec("copper_oxidation", &[])));
        assert!(!filter.matches(&make_spec("copper_waxing_test", &[])));
    }

    #[test]
    fn test_filter_by_tags_single() {
        let filter = TestFilter::by_tags(["redstone"]);

        assert!(filter.matches(&make_spec("test1", &["redstone"])));
        assert!(filter.matches(&make_spec("test2", &["redstone", "other"])));
        assert!(!filter.matches(&make_spec("test3", &["copper"])));
        assert!(!filter.matches(&make_spec("test4", &[])));
    }

    #[test]
    fn test_filter_by_tags_multiple() {
        let filter = TestFilter::by_tags(["redstone", "copper"]);

        // Matches if test has ANY of the specified tags
        assert!(filter.matches(&make_spec("test1", &["redstone"])));
        assert!(filter.matches(&make_spec("test2", &["copper"])));
        assert!(filter.matches(&make_spec("test3", &["redstone", "copper"])));
        assert!(!filter.matches(&make_spec("test4", &["iron"])));
    }

    #[test]
    fn test_filter_by_pattern_star() {
        let filter = TestFilter::by_patterns(["copper_*"]);

        assert!(filter.matches(&make_spec("copper_waxing", &[])));
        assert!(filter.matches(&make_spec("copper_oxidation", &[])));
        assert!(filter.matches(&make_spec("copper_", &[])));
        assert!(!filter.matches(&make_spec("iron_block", &[])));
        assert!(!filter.matches(&make_spec("coppertest", &[])));
    }

    #[test]
    fn test_filter_by_pattern_question() {
        let filter = TestFilter::by_patterns(["test_?"]);

        assert!(filter.matches(&make_spec("test_1", &[])));
        assert!(filter.matches(&make_spec("test_a", &[])));
        assert!(!filter.matches(&make_spec("test_12", &[])));
        assert!(!filter.matches(&make_spec("test_", &[])));
    }

    #[test]
    fn test_filter_by_pattern_combined() {
        let filter = TestFilter::by_patterns(["*_test_*"]);

        assert!(filter.matches(&make_spec("copper_test_1", &[])));
        assert!(filter.matches(&make_spec("redstone_test_basic", &[])));
        assert!(!filter.matches(&make_spec("test_basic", &[])));
    }

    #[test]
    fn test_filter_multiple_patterns() {
        let filter = TestFilter::by_patterns(["copper_*", "redstone_*"]);

        assert!(filter.matches(&make_spec("copper_waxing", &[])));
        assert!(filter.matches(&make_spec("redstone_repeater", &[])));
        assert!(!filter.matches(&make_spec("iron_block", &[])));
    }

    #[test]
    fn test_filter_combined_tags_and_patterns() {
        let filter = TestFilter::all()
            .with_tags(["redstone"])
            .with_patterns(["*_test"]);

        // Must match both: has "redstone" tag AND name ends with "_test"
        assert!(filter.matches(&make_spec("repeater_test", &["redstone"])));
        assert!(!filter.matches(&make_spec("repeater_test", &["copper"]))); // wrong tag
        assert!(!filter.matches(&make_spec("repeater", &["redstone"]))); // wrong pattern
    }

    #[test]
    fn test_filter_builder_pattern() {
        let filter = TestFilter::all()
            .with_tags(["a", "b"])
            .with_patterns(["test_*"])
            .with_exact_name("test_specific");

        assert_eq!(filter.tags, vec!["a", "b"]);
        assert_eq!(filter.name_patterns, vec!["test_*"]);
        assert_eq!(filter.exact_name, Some("test_specific".to_string()));
    }

    // ==========================================================================
    // Glob Matching Tests
    // ==========================================================================

    #[test]
    fn test_glob_exact_match() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
        assert!(!glob_match("hello", "hello_world"));
    }

    #[test]
    fn test_glob_star_end() {
        assert!(glob_match("hello*", "hello"));
        assert!(glob_match("hello*", "hello_world"));
        assert!(glob_match("hello*", "hellooooo"));
        assert!(!glob_match("hello*", "hell"));
    }

    #[test]
    fn test_glob_star_start() {
        assert!(glob_match("*world", "world"));
        assert!(glob_match("*world", "hello_world"));
        assert!(!glob_match("*world", "worldly"));
    }

    #[test]
    fn test_glob_star_middle() {
        assert!(glob_match("hello*world", "helloworld"));
        assert!(glob_match("hello*world", "hello_world"));
        assert!(glob_match("hello*world", "hello_big_world"));
        assert!(!glob_match("hello*world", "hello_worlds"));
    }

    #[test]
    fn test_glob_multiple_stars() {
        assert!(glob_match("*a*b*", "ab"));
        assert!(glob_match("*a*b*", "xaxbx"));
        assert!(glob_match("*a*b*", "aaabbb"));
        assert!(!glob_match("*a*b*", "ba"));
    }

    #[test]
    fn test_glob_question_mark() {
        assert!(glob_match("h?llo", "hello"));
        assert!(glob_match("h?llo", "hallo"));
        assert!(!glob_match("h?llo", "hllo"));
        assert!(!glob_match("h?llo", "heello"));
    }

    #[test]
    fn test_glob_mixed() {
        assert!(glob_match("test_?_*", "test_1_abc"));
        assert!(glob_match("test_?_*", "test_a_"));
        assert!(!glob_match("test_?_*", "test__abc"));
    }

    #[test]
    fn test_glob_empty() {
        assert!(glob_match("", ""));
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("?", ""));
    }
}
