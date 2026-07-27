//! Bounded Codex session-log extraction for rate limits fallback.
//!
//! Scans `.codex/sessions/**/*.jsonl` for the latest `token_count` event that
//! carries a `rate_limits` object, then re-serializes that object for
//! [`crate::providers::v2_map::codex_from_rate_limits_json`].

use std::path::Path;

/// Reverse-scan JSONL bytes; first `payload.type == "token_count"` with a
/// `payload.rate_limits` object wins. Returns re-serialized rate_limits JSON.
pub fn extract_rate_limits_from_jsonl(bytes: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(bytes).ok()?;
    for line in text.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(payload) = value.get("payload") else {
            continue;
        };
        let payload_type = payload.get("type").and_then(|v| v.as_str());
        if payload_type != Some("token_count") {
            continue;
        }
        let Some(rate_limits) = payload.get("rate_limits") else {
            continue;
        };
        if !rate_limits.is_object() {
            continue;
        }
        return serde_json::to_vec(rate_limits).ok();
    }
    None
}

/// Per-file read cap for session JSONL candidates (1 MiB).
const MAX_SESSION_JSONL_BYTES: u64 = 1024 * 1024;

/// Bounded walk of `sessions_dir`: no symlinks, depth ≤ 8, visits ≤ 4096,
/// candidates ≤ 256 jsonl files, each file ≤ 1 MiB. Sort mtime desc then path
/// asc; scan each candidate reverse for token_count; return first hit's
/// rate_limits JSON.
pub fn find_latest_rate_limits(sessions_dir: &Path) -> Option<Vec<u8>> {
    use std::cmp::Ordering;
    use std::fs;

    if !sessions_dir.is_dir() {
        return None;
    }

    let mut candidates: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
    let mut stack = vec![(sessions_dir.to_path_buf(), 0u32)];
    let mut visits = 0u32;

    while let Some((dir, depth)) = stack.pop() {
        if visits >= 4096 || depth > 8 {
            continue;
        }
        visits += 1;
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visits += 1;
            if visits >= 4096 {
                break;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() && depth < 8 {
                stack.push((path, depth + 1));
            } else if meta.is_file()
                && path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
            {
                let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                candidates.push((mtime, path));
            }
        }
    }

    candidates.sort_by(|a, b| match b.0.cmp(&a.0) {
        Ordering::Equal => a.1.as_os_str().cmp(b.1.as_os_str()),
        other => other,
    });
    candidates.truncate(256);

    // Prefer newest candidate that yields extractable limits (not merely
    // newest file without limits). Skip files larger than 1 MiB.
    for (_mtime, path) in &candidates {
        let Ok(meta) = fs::metadata(path) else {
            continue;
        };
        if meta.len() > MAX_SESSION_JSONL_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        if let Some(raw) = extract_rate_limits_from_jsonl(&bytes) {
            return Some(raw);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::v2_map::codex_from_rate_limits_json;
    use crate::status::schema::ProviderResult;
    use time::macros::datetime;

    #[test]
    fn extract_token_count_rate_limits_from_jsonl() {
        let bytes =
            include_bytes!("../../tests/fixtures/providers/codex/session-token-count.jsonl");
        let raw = extract_rate_limits_from_jsonl(bytes).expect("limits");
        let now = datetime!(2026-07-26 18:00:00 UTC);
        match codex_from_rate_limits_json(&raw, now) {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "weekly");
                assert!((windows[0].used_percent() - 12.5).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn find_latest_rate_limits_from_temp_sessions_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sessions = dir.path().join("sessions");
        let nested = sessions.join("2026/07/25");
        std::fs::create_dir_all(&nested).expect("mkdir");
        let fixture =
            include_bytes!("../../tests/fixtures/providers/codex/session-token-count.jsonl");
        std::fs::write(nested.join("rollout.jsonl"), fixture).expect("write");

        let raw = find_latest_rate_limits(&sessions).expect("limits from walk");
        let now = datetime!(2026-07-26 18:00:00 UTC);
        match codex_from_rate_limits_json(&raw, now) {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "weekly");
                assert!((windows[0].used_percent() - 12.5).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn extract_skips_non_token_count_and_prefers_last_match() {
        let jsonl = concat!(
            r#"{"payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":1.0,"window_minutes":10080}}}}"#,
            "\n",
            r#"{"payload":{"type":"message"}}"#,
            "\n",
            r#"{"payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":99.0,"window_minutes":10080}}}}"#,
            "\n",
        );
        let raw = extract_rate_limits_from_jsonl(jsonl.as_bytes()).expect("limits");
        let now = datetime!(2026-07-26 18:00:00 UTC);
        match codex_from_rate_limits_json(&raw, now) {
            ProviderResult::Ready { windows, .. } => {
                // Reverse scan → last token_count line wins.
                assert!((windows[0].used_percent() - 99.0).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn find_latest_skips_jsonl_without_rate_limits() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("mkdir");
        // File without usable limits (sorts first by path when mtimes match).
        std::fs::write(
            sessions.join("empty.jsonl"),
            b"{\"payload\":{\"type\":\"message\"}}\n",
        )
        .expect("write empty");
        let fixture =
            include_bytes!("../../tests/fixtures/providers/codex/session-token-count.jsonl");
        std::fs::write(sessions.join("good.jsonl"), fixture).expect("write good");

        let raw = find_latest_rate_limits(&sessions).expect("skip empty, use good");
        let now = datetime!(2026-07-26 18:00:00 UTC);
        match codex_from_rate_limits_json(&raw, now) {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "weekly");
                assert!((windows[0].used_percent() - 12.5).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn find_latest_skips_files_over_1_mib_in_favor_of_smaller_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("mkdir");

        // Oversized candidate sorts first by path ("a-huge.jsonl" < "z-good.jsonl")
        // when mtimes match; must be skipped due to 1 MiB cap.
        let huge_path = sessions.join("a-huge.jsonl");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&huge_path).expect("create huge");
            // Write just over 1 MiB of JSONL-shaped content with a fake rate_limits
            // so a size-blind reader would succeed — size cap must prevent that.
            let line = concat!(
                r#"{"payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":1.0,"window_minutes":10080}}}}"#,
                "\n"
            );
            let mut written = 0usize;
            let target = (MAX_SESSION_JSONL_BYTES as usize) + 1;
            while written < target {
                f.write_all(line.as_bytes()).expect("write chunk");
                written += line.len();
            }
            f.flush().expect("flush");
        }
        assert!(
            std::fs::metadata(&huge_path).expect("meta").len() > MAX_SESSION_JSONL_BYTES,
            "fixture must exceed 1 MiB"
        );

        let fixture =
            include_bytes!("../../tests/fixtures/providers/codex/session-token-count.jsonl");
        std::fs::write(sessions.join("z-good.jsonl"), fixture).expect("write good");

        let raw = find_latest_rate_limits(&sessions).expect("skip huge, use good");
        let now = datetime!(2026-07-26 18:00:00 UTC);
        match codex_from_rate_limits_json(&raw, now) {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "weekly");
                assert!((windows[0].used_percent() - 12.5).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }
}
