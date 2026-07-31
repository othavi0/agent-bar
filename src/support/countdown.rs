//! Human countdown formatting, shared with the popup.
//!
//! `CoreView.countdownText` renders the same durations in QML on a
//! 30-second timer and must produce byte-identical strings. Neither side can
//! call the other (the product ships no JS runtime), so both are pinned to
//! `tests/fixtures/countdown-table.json`; see `tests/countdown_parity.rs`.

use time::{Duration, OffsetDateTime};

/// `"2d 18h"` | `"3h 1m"` | `"5m"`. Truncates like the QML original: whole
/// minutes first, then whole days and hours out of those. Never rounds up, so
/// a countdown never claims more time than remains.
///
/// The parity contract with `CoreView.countdownText` holds for non-negative
/// durations only: a negative `remaining` clamps to `"0m"` here, where the
/// QML original's `Math.floor` would keep counting past zero into negative
/// minutes. Neither side can reach that input in the product today — this
/// side is only ever called through `reset_countdown`, which returns `"now"`
/// before a negative duration gets here, and the QML side is only ever
/// called through `resetCountdownText`, which does the same. A caller that
/// skips that sign check would see this function and the popup disagree.
pub fn countdown_text(remaining: Duration) -> String {
    let total_minutes = remaining.whole_minutes().max(0);
    let days = total_minutes / 1440;
    let hours = (total_minutes % 1440) / 60;
    let minutes = total_minutes % 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// Mirrors `CoreView.resetCountdownText` for a window that has a timestamp:
/// the literal `"now"` once the reset has passed, otherwise the countdown.
/// A window with no timestamp never reaches here — the caller renders nothing.
pub fn reset_countdown(now: OffsetDateTime, reset_at: OffsetDateTime) -> String {
    let remaining = reset_at - now;
    if !remaining.is_positive() {
        return "now".to_owned();
    }
    countdown_text(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn reset_in_the_past_reads_now() {
        let now = datetime!(2026-07-28 15:00:00 UTC);
        assert_eq!(
            reset_countdown(now, datetime!(2026-07-28 14:00:00 UTC)),
            "now"
        );
        assert_eq!(reset_countdown(now, now), "now");
    }

    #[test]
    fn reset_in_the_future_counts_down() {
        let now = datetime!(2026-07-28 15:00:00 UTC);
        assert_eq!(
            reset_countdown(now, datetime!(2026-07-28 18:01:00 UTC)),
            "3h 1m"
        );
        assert_eq!(
            reset_countdown(now, datetime!(2026-07-31 09:00:00 UTC)),
            "2d 18h"
        );
    }

    #[test]
    fn seconds_never_round_the_minute_up() {
        // 59 seconds short of an hour is 59m, not 1h — the popup would
        // otherwise promise a reset that has not arrived.
        let now = datetime!(2026-07-28 15:00:00 UTC);
        assert_eq!(
            reset_countdown(now, datetime!(2026-07-28 15:59:59 UTC)),
            "59m"
        );
    }
}
