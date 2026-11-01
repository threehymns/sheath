//! Wall-clock readings for the row labels.
//!
//! Native builds read the system clock. A browser tab has no `chrono` clock behind it, so the
//! web build asks JavaScript for the time; the browser has no timezone database either, so it
//! labels rows in UTC.

use chrono::{DateTime, Local};

/// The current time, read once per frame by the caller.
pub fn now() -> DateTime<Local> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Local::now()
    }
    #[cfg(target_arch = "wasm32")]
    {
        use chrono::TimeZone;
        let millis = js_sys::Date::now() as i64;
        Local
            .timestamp_millis_opt(millis)
            .single()
            .unwrap_or_else(|| Local.timestamp_millis_opt(0).unwrap())
    }
}
