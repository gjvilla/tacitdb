//! PyO3 shell over `tacit-core`. Bindings land after the core API stabilizes
//! (D-0015); this crate exists so the boundary stays a separate compilation
//! unit and `tacit-core` never grows a Python dependency.

pub use tacit_core;
