mod fixtures_golden;
mod full_feed;
mod invoice;
mod peaks_io;

use crate::common::{fixture_in, fixtures_dir_in};
use std::path::PathBuf;

const MODULE_NAME: &str = "green_button";

pub fn fixture(name: &str) -> PathBuf {
    fixture_in(MODULE_NAME, name)
}

pub fn fixtures_dir() -> PathBuf {
    fixtures_dir_in(MODULE_NAME)
}
