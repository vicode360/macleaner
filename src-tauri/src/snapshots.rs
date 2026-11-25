//! Local APFS Time Machine snapshot inspection and cleanup via `tmutil`.

use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::process::Command;

use serde::{Deserialize, Serialize};

const TM_PREFIX: &str = "com.apple.TimeMachine.";
const TM_SUFFIX: &str = ".local";
const DEFAULT_VOLUME: &str = "/";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApfsSnapshot {
    pub id: String,
    pub date_token: String,
    pub timestamp_iso: String,
    pub volume: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotListResult {
    pub snapshots: Vec<ApfsSnapshot>,
    pub supported: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDeleteFailure {
    pub date_token: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDeleteResult {
    pub deleted_tokens: Vec<String>,
    pub failed: Vec<SnapshotDeleteFailure>,
    pub cancelled: bool,
}

/// Validates a `tmutil` local snapshot timestamp token (`YYYY-MM-DD-HHMMSS`).
#[must_use]
pub(crate) fn is_valid_date_token(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.len() != 17 || !token.is_ascii() {
        return false;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'-' {
        return false;
    }

    let digit_positions = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 13, 14, 15, 16];
    if !digit_positions.iter().all(|&i| bytes[i].is_ascii_digit()) {
        return false;
    }

    let Ok(year) = token[0..4].parse::<u16>() else {
        return false;
    };
    let Ok(month) = token[5..7].parse::<u8>() else {
        return false;
    };
    let Ok(day) = token[8..10].parse::<u8>() else {
        return false;
    };
    let Ok(hour) = token[11..13].parse::<u8>() else {
        return false;
    };
    let Ok(minute) = token[13..15].parse::<u8>() else {
        return false;
    };
    let Ok(second) = token[15..17].parse::<u8>() else {
        return false;
    };

    (1970..=9999).contains(&year)
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour <= 23
        && minute <= 59
        && second <= 59
}

#[must_use]
pub(crate) fn format_iso_timestamp(token: &str) -> Option<String> {
    if !is_valid_date_token(token) {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}:{}:{}",
        &token[0..4],
        &token[5..7],
        &token[8..10],
        &token[11..13],
        &token[13..15],
        &token[15..17]
    ))
}

#[must_use]
pub(crate) fn parse_tmutil_snapshots(stdout: &str) -> Vec<ApfsSnapshot> {
    let mut seen = HashSet::new();
    let mut snapshots = Vec::new();

    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        let Some(rest) = line.strip_prefix(TM_PREFIX) else {
            continue;
        };
        let Some(date_token) = rest.strip_suffix(TM_SUFFIX) else {
            continue;
        };
        let Some(timestamp_iso) = format_iso_timestamp(date_token) else {
            continue;
        };
        if !seen.insert(date_token.to_string()) {
            continue;
        }
        snapshots.push(ApfsSnapshot {
            id: line.to_string(),
            date_token: date_token.to_string(),
            timestamp_iso,
            volume: DEFAULT_VOLUME.to_string(),
        });
    }

    snapshots.sort_by(|a, b| b.date_token.cmp(&a.date_token));
    snapshots
}

#[cfg(any(target_os = "macos", test))]
fn build_privileged_delete_script(tokens: &[String]) -> Option<String> {
    if tokens.is_empty() || !tokens.iter().all(|token| is_valid_date_token(token)) {
        return None;
    }
    let commands: Vec<String> = tokens
        .iter()
        .map(|token| format!("/usr/bin/tmutil deletelocalsnapshots {token}"))
        .collect();
    Some(format!(
        "do shell script \"{}\" with administrator privileges",
        commands.join("; ")
    ))
}

#[cfg(any(target_os = "macos", test))]
fn is_user_cancelled_osascript(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("user canceled") || lower.contains("(-128)")
}

#[cfg(target_os = "macos")]
fn combined_output_message(stdout: &[u8], stderr: &[u8]) -> String {
    let err = String::from_utf8_lossy(stderr).trim().to_string();
    if !err.is_empty() {
        return err;
    }
    String::from_utf8_lossy(stdout).trim().to_string()
}

#[cfg(target_os = "macos")]
pub(crate) fn list_local_snapshots() -> Result<SnapshotListResult, String> {
    let output = Command::new("/usr/bin/tmutil")
        .args(["listlocalsnapshots", DEFAULT_VOLUME])
        .output()
        .map_err(|err| format!("failed to run tmutil: {err}"))?;

    if !output.status.success() {
        let msg = combined_output_message(&output.stdout, &output.stderr);
        return Err(if msg.is_empty() {
            format!("tmutil exited with status {}", output.status)
        } else {
            msg
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let snapshots = parse_tmutil_snapshots(&stdout);

    Ok(SnapshotListResult {
        snapshots,
        supported: true,
        note: None,
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn list_local_snapshots() -> Result<SnapshotListResult, String> {
    Ok(SnapshotListResult {
        snapshots: Vec::new(),
        supported: false,
        note: Some("APFS local snapshots are only available on macOS.".to_string()),
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn delete_local_snapshots(
    date_tokens: &[String],
) -> Result<SnapshotDeleteResult, String> {
    let mut seen = HashSet::new();
    let mut valid_tokens = Vec::new();
    let mut failed = Vec::new();

    for raw in date_tokens {
        let token = raw.trim();
        if !is_valid_date_token(token) {
            failed.push(SnapshotDeleteFailure {
                date_token: raw.clone(),
                reason: "Identificador de snapshot inválido.".to_string(),
            });
            continue;
        }
        if seen.insert(token.to_string()) {
            valid_tokens.push(token.to_string());
        }
    }

    if valid_tokens.is_empty() {
        return Ok(SnapshotDeleteResult {
            deleted_tokens: Vec::new(),
            failed,
            cancelled: false,
        });
    }

    let mut deleted_tokens = Vec::new();
    let mut needs_elevation = Vec::new();

    for token in valid_tokens {
        let output = Command::new("/usr/bin/tmutil")
            .args(["deletelocalsnapshots", &token])
            .output()
            .map_err(|err| format!("failed to run tmutil: {err}"))?;

        if output.status.success() {
            deleted_tokens.push(token);
            continue;
        }

        let detail = combined_output_message(&output.stdout, &output.stderr);
        let lower = detail.to_lowercase();
        if lower.contains("no local snapshot") || lower.contains("not found") {
            failed.push(SnapshotDeleteFailure {
                date_token: token,
                reason: detail,
            });
        } else {
            needs_elevation.push((token, detail));
        }
    }

    if needs_elevation.is_empty() {
        return Ok(SnapshotDeleteResult {
            deleted_tokens,
            failed,
            cancelled: false,
        });
    }

    let retry_tokens: Vec<String> = needs_elevation
        .iter()
        .map(|(token, _)| token.clone())
        .collect();
    let Some(script) = build_privileged_delete_script(&retry_tokens) else {
        return Err("invalid snapshot token in elevation batch".to_string());
    };

    let elevated = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .map_err(|err| format!("failed to request macOS administrator privileges: {err}"))?;

    let elevated_msg = combined_output_message(&elevated.stdout, &elevated.stderr);
    if !elevated.status.success() && is_user_cancelled_osascript(&elevated_msg) {
        return Ok(SnapshotDeleteResult {
            deleted_tokens,
            failed,
            cancelled: true,
        });
    }

    let remaining_tokens: HashSet<String> = list_local_snapshots()
        .map(|res| res.snapshots.into_iter().map(|s| s.date_token).collect())
        .unwrap_or_default();

    for (token, initial_err) in needs_elevation {
        if !remaining_tokens.contains(&token) {
            deleted_tokens.push(token);
        } else {
            let reason = if !elevated_msg.is_empty() {
                elevated_msg.clone()
            } else if !initial_err.is_empty() {
                initial_err
            } else {
                "Não foi possível remover o snapshot APFS.".to_string()
            };
            failed.push(SnapshotDeleteFailure {
                date_token: token,
                reason,
            });
        }
    }

    Ok(SnapshotDeleteResult {
        deleted_tokens,
        failed,
        cancelled: false,
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn delete_local_snapshots(
    date_tokens: &[String],
) -> Result<SnapshotDeleteResult, String> {
    Ok(SnapshotDeleteResult {
        deleted_tokens: Vec::new(),
        failed: date_tokens
            .iter()
            .map(|token| SnapshotDeleteFailure {
                date_token: token.clone(),
                reason: "APFS local snapshots are only available on macOS.".to_string(),
            })
            .collect(),
        cancelled: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_time_machine_local_snapshots_sorted_newest_first() {
        let sample = "\
Snapshots for disk /:
com.apple.TimeMachine.2026-11-20-091500.local
com.apple.os.update-9A3B4C5D6E7F
com.bombich.ccc.2026-11-24-120000.local
com.apple.TimeMachine.2026-11-24-183012.local
com.apple.TimeMachine.2026-11-24-183012.local
com.apple.TimeMachine.invalid-token.local
";
        let parsed = parse_tmutil_snapshots(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].date_token, "2026-11-24-183012");
        assert_eq!(parsed[0].timestamp_iso, "2026-11-24T18:30:12");
        assert_eq!(
            parsed[0].id,
            "com.apple.TimeMachine.2026-11-24-183012.local"
        );
        assert_eq!(parsed[1].date_token, "2026-11-20-091500");
        assert_eq!(parsed[1].timestamp_iso, "2026-11-20T09:15:00");
    }

    #[test]
    fn date_token_validation_blocks_injection_and_out_of_range_values() {
        assert!(is_valid_date_token("2026-11-24-183012"));
        assert!(is_valid_date_token("2025-01-01-000000"));
        assert!(!is_valid_date_token("2026-13-24-183012"));
        assert!(!is_valid_date_token("2026-11-32-183012"));
        assert!(!is_valid_date_token("2026-11-24-240000"));
        assert!(!is_valid_date_token("2026-11-24-183012; rm -rf /"));
        assert!(!is_valid_date_token("2026-11-24-183012\""));
        assert!(!is_valid_date_token("../2026-11-24-183012"));
        assert!(!is_valid_date_token(""));
    }

    #[test]
    fn privileged_script_builder_rejects_unvalidated_tokens() {
        let valid = vec![
            "2026-11-24-183012".to_string(),
            "2026-11-23-120000".to_string(),
        ];
        let script = build_privileged_delete_script(&valid).expect("builds script");
        assert_eq!(
            script,
            "do shell script \"/usr/bin/tmutil deletelocalsnapshots 2026-11-24-183012; /usr/bin/tmutil deletelocalsnapshots 2026-11-23-120000\" with administrator privileges"
        );

        let unsafe_tokens = vec!["2026-11-24-183012; id".to_string()];
        assert!(build_privileged_delete_script(&unsafe_tokens).is_none());
        assert!(build_privileged_delete_script(&[]).is_none());
    }

    #[test]
    fn detects_macos_osascript_user_cancellation() {
        assert!(is_user_cancelled_osascript(
            "execution error: User canceled. (-128)"
        ));
        assert!(!is_user_cancelled_osascript(
            "Failed to delete local snapshot"
        ));
    }
}
