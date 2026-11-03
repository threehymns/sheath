//! Tests for the terminal pane, grouped by what each one checks: the pane's own numbers, the cell
//! it measures, the frame it paints, the shell behind it, and the tables the cell-filling
//! characters come from.

mod box_drawing;
mod capabilities;
mod images;
mod integration;
mod metrics;
mod mouse;
mod osc;
mod paint;
mod pane;
mod shell;
mod wide;

#[cfg(not(target_arch = "wasm32"))]
mod perf;
