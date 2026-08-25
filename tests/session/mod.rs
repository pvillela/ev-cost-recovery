mod consistency_band;
mod report_rendering;
mod segment_tiling;

use crate::common::{fixture_in, fixtures_dir_in};
use std::path::PathBuf;

const MODULE_NAME: &str = "sessions";

pub fn fixture(name: &str) -> PathBuf {
    fixture_in(MODULE_NAME, name)
}

pub fn fixtures_dir() -> PathBuf {
    fixtures_dir_in(MODULE_NAME)
}
