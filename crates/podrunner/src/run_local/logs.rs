//! The logs of a local run (T-134): each new part of a log as it comes, on
//! stderr with the job as its prefix (or as JSON with `--json`); `--log-dir
//! DIR` keeps the log of each job and step and the summary of each job.
//! podrunner masks each secret again before it writes a line: the runner
//! masks them too, and a secret must never reach a file of the user.

use std::path::Path;
use std::sync::Arc;

use podrunner_local::drive::Line;
use podrunner_local::exec::Live;
use podrunner_schema::JobId;
use serde_json::json;

/// The fix when a failed job gave the made-up `github.token` to a real API
/// (T-254), else nothing.
pub(super) fn token_hint<J>(logs: &[(J, String)]) -> String {
    // A real API refuses the made-up token: the API with "Bad credentials",
    // raw.githubusercontent.com with a 404 (the manifest of setup-python,
    // T-269).
    let refused = |log: &str| {
        log.contains("Bad credentials")
            || (log.contains("Unable to get manifest from https://raw.githubusercontent.com"))
    };
    match logs.iter().any(|(_, log)| refused(log)) {
        true => "podrunner: a job gave the made-up github.token to a real API, which refused it; fix: give a token with --github-token-env NAME (a token with read access to public repositories is enough); an empty NAME sends no token, which the manifests of setup-* take, but an action that requires a token input (actions/github-script) refuses it\n".into(),
        false => String::new(),
    }
}

/// `text` with each secret value as `***`.
pub(super) fn mask(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    for s in secrets.iter().filter(|s| !s.is_empty()) {
        out = out.replace(s.as_str(), "***");
    }
    out
}

/// The live sink: one line on stderr for each line of a log.
pub(super) fn live(secrets: Vec<String>, as_json: bool) -> Live {
    Arc::new(move |job: &JobId, text: &str| {
        for line in mask(text, &secrets).lines() {
            if as_json {
                eprintln!("{}", json!({ "event": "log", "job": job.to_string(), "line": line }));
            } else {
                eprintln!("[{job}] {line}");
            }
        }
    })
}

/// One JSON object for the end of a job.
pub(super) fn line_json(l: &Line) -> String {
    let v = match l {
        Line::Ran { job, name, result } => {
            json!({ "event": "job", "job": job.to_string(), "name": name, "result": result.as_str() })
        }
        Line::Refused { job, name, reason } => {
            json!({ "event": "job", "job": job.to_string(), "name": name, "result": "failure", "reason": reason })
        }
        Line::Skipped { job, condition } => {
            json!({ "event": "job", "name": job, "result": "skipped", "if": condition })
        }
        Line::Kept { job, name, result } => {
            json!({ "event": "job", "job": job.to_string(), "name": name, "result": result.as_str(), "kept": true })
        }
        Line::NotSelected { job, name, reason } => {
            json!({ "event": "job", "job": job.to_string(), "name": name, "result": "skipped", "reason": reason })
        }
    };
    format!("{v}\n")
}

/// Write `DIR/JOB/step-ID.log` for each step log, `DIR/JOB/job.log` for the
/// rest, and `DIR/JOB/summary.md` of the summaries of the job.
pub(super) fn write_dir(
    dir: &Path,
    blobs: &[(JobId, Vec<(String, String)>)],
    secrets: &[String],
) -> Result<(), String> {
    for (job, list) in blobs {
        let d = dir.join(job.as_str());
        std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        let mut summary = String::new();
        let mut rest = String::new();
        for (name, text) in list {
            let text = mask(text, secrets);
            if name.starts_with("summary-") {
                summary.push_str(&text);
            } else if name.starts_with("step-") && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                let f = d.join(format!("{name}.log"));
                std::fs::write(&f, text).map_err(|e| format!("{}: {e}", f.display()))?;
            } else {
                rest.push_str(&text);
            }
        }
        for (file, text) in [("summary.md", summary), ("job.log", rest)] {
            if !text.is_empty() {
                let f = d.join(file);
                std::fs::write(&f, text).map_err(|e| format!("{}: {e}", f.display()))?;
            }
        }
    }
    Ok(())
}

/// Write each result line to stdout when its job ends, not in the `Out` of
/// the run: the binary sets it; a test reads the lines in `Out` (T-296).
pub static STREAM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Add `text` to `out`, or with [`STREAM`] write it to stdout at once.
pub(super) fn emit(out: &mut String, text: &str) {
    if STREAM.load(std::sync::atomic::Ordering::SeqCst) {
        use std::io::Write;
        let mut o = std::io::stdout().lock();
        let _ = o.write_all(text.as_bytes());
        let _ = o.flush();
    } else {
        out.push_str(text);
    }
}

/// The end of stderr of a failed run. The live lines gave each log once;
/// with `-q`, the logs come here (T-289), as JSON with `--json` (T-296).
pub(super) fn failed_end<J: std::fmt::Display>(logs: &[(J, String)], quiet: bool, json: bool, dry: bool) -> String {
    let mut out = String::new();
    match (quiet, json) {
        (true, true) => {
            for (job, log) in logs {
                for l in log.lines() {
                    out.push_str(&format!("{}\n", json!({ "event": "log", "job": job.to_string(), "line": l })));
                }
            }
        }
        (true, false) => {
            for (job, log) in logs {
                out.push_str(&format!("--- log of {job}\n{log}\n"));
            }
        }
        (false, _) if dry => {}
        (false, _) => out.push_str("podrunner: a job failed; its log is above (and in --log-dir DIR)\n"),
    }
    out
}

#[cfg(test)]
mod hint_tests {
    /// T-254: a failed job with "Bad credentials" in its log names the fix.
    #[test]
    fn bad_credentials_name_the_token_flag() {
        let logs = vec![("a".to_string(), "##[error]Last error: Bad credentials".to_string())];
        assert!(super::token_hint(&logs).contains("--github-token-env"));
        assert!(super::token_hint(&[("a".to_string(), "other".to_string())]).is_empty());
        let python = "##[error]Failed to fetch the Python versions manifest. Unable to get manifest from https://raw.githubusercontent.com/actions/python-versions/main/versions-manifest.json";
        assert!(super::token_hint(&[("a".to_string(), python.to_string())]).contains("an empty NAME"));
    }
}
