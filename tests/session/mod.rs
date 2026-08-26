mod report_rendering;
mod site_load_golden;

use crate::common::fixtures_dir_in;
use std::path::PathBuf;

const MODULE_NAME: &str = "sessions";

// No `fixture` helper here, unlike the other test modules: nothing under `tests/session/` opens a
// fixture *input* any more. The tests that do live in `src/session/`, where the estimating call
// they render is reachable, and reach `tests/fixtures/` through `golden::fixture`.
pub fn fixtures_dir() -> PathBuf {
    fixtures_dir_in(MODULE_NAME)
}
