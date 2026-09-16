//! What a front-end asks the library, stated in the terms a front-end has.
//!
//! The rest of the crate is organised by the source of data it reads. A caller checking an invoice
//! has none of those in hand — it has a billing period, a meter export and the charging network's
//! monthly reports — so this module is where those become the calls the other modules understand.
//!
//! # How it is arranged
//!
//! [`pure`] computes; the private `io` reads, and its contents are re-exported here. Every `io`
//! function turns paths into values and delegates, so that the reasoning a figure rests on can be
//! exercised without a filesystem, and so that one file answers what the library touches on disk.
//!
//! Inside the crate the split is three files — `io`, `pure` and `error`. From outside it is one
//! flat namespace plus [`pure`]: a caller writes [`peak_power`], [`ApiError`] and [`ReadError`]
//! without choosing which of the three declares them, and reaches the pure counterpart of a
//! reading call as [`pure::peak_power`]. `error` is private because the union it holds belongs to
//! both halves rather than to a subject of its own, so a path segment naming it would say nothing
//! a caller needs to know.
//!
//! [`pure`] is the exception that stays a module, because its whole point is a namespace a caller
//! can reach without a filesystem behind it. Its own submodules are private for the same reason
//! `io` and `error` are: they divide the code by subject, which is a fact about maintaining it
//! rather than about calling it.
//!
//! # What each module re-exports
//!
//! The rule is that a module re-exports every type a caller must be able to *name* in order to use
//! its public items — for a function, its parameters and return type; for an enum, its variant
//! payloads, since matching past the first level forces the caller to write them. Calling a
//! function therefore never requires knowing which module it delegates to.
//!
//! Field types are excluded, because reading or destructuring a field never requires naming it.
//! Probing *into* a returned type may well lead elsewhere in the crate — the two halves of
//! [`PowerEstimates`] are [`session::IntervalEstimates`](crate::session::IntervalEstimates), and
//! reading about them means going there. Re-exporting transitively would put the whole crate in
//! every module.

mod error;
pub use error::*;

mod io;
pub use io::*;

pub mod pure;
