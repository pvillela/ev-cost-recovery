// No `invoice` module: the invoice reconciliation needs the crate-private `period_values`, so it
// lives in `src/green_button/invoice_tests.rs`.
mod fixtures_golden;
mod full_feed;
mod read_xml;

use crate::common::{fixture_in, fixtures_dir_in};
use std::path::PathBuf;

const MODULE_NAME: &str = "green_button";

pub fn fixture(name: &str) -> PathBuf {
    fixture_in(MODULE_NAME, name)
}

pub fn fixtures_dir() -> PathBuf {
    fixtures_dir_in(MODULE_NAME)
}
