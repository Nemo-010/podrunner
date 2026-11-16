//! The help of each command (T-208): `podrunner help [COMMAND]`,
//! `COMMAND --help` (or `-h`) and `podrunner version`. Each answer is exit 0
//! and needs no other argument.

use pod_core::failure::Class;

use crate::cli::{Out, USAGE, err, ok};
use crate::text::display;

/// The page of each option, for a user who has no tree (T-278).
const CLI_DOC: &str = concat!("https://github.com/PodUtils/podrunner/blob/", env!("PODRUNNER_DOC_REF"), "/docs/cli.md");

/// The answer to `help`, `version` or a `--help` of a command; `None` for
/// each other command line.
pub(crate) fn answer(cmd: &str, rest: &[String]) -> Option<Out> {
    match cmd {
        "version" => return Some(ok(crate::cli::version())),
        "help" => {
            return Some(match rest.first() {
                None => ok(USAGE.into()),
                Some(c) => of(c)
                    .map(ok)
                    .unwrap_or_else(|| err(Class::Usage, format!("an unknown command {:?}\n{USAGE}", display(c)))),
            });
        }
        _ => {}
    }
    // The arguments of a program after `--` are not options of podrunner.
    let asks = rest.iter().take_while(|a| *a != "--").any(|a| a == "--help" || a == "-h");
    if asks { of(cmd).map(ok) } else { None }
}

/// The usage of one command: its lines of `USAGE`, or the options of `run`.
fn of(cmd: &str) -> Option<String> {
    if cmd == "run" {
        return Some(wrap(crate::run_local::USAGE));
    }
    let mut out = String::new();
    let mut inside = false;
    for line in USAGE.lines() {
        // A command starts two spaces in; its next lines start four or more.
        if line.strip_prefix("  ").is_some_and(|l| l.split_whitespace().next() == Some(cmd)) {
            inside = true;
            // The usage on its line, its description under it (T-278).
            let l = line.trim_start();
            match l.split_once("   ") {
                Some((usage, what)) => out.push_str(&format!("usage: podrunner {usage}\n    {}\n", what.trim())),
                None => out.push_str(&format!("usage: podrunner {l}\n")),
            }
        } else if inside && line.starts_with("    ") {
            out.push_str(&format!("{line}\n"));
        } else {
            inside = false;
        }
    }
    if cmd == "doctor" {
        out.push_str("The options of doctor --run are those of run: podrunner run --help\n");
    }
    (!out.is_empty()).then_some(out)
}

/// The usage of `run`, one option group to a word, in lines of 96 columns.
fn wrap(usage: &str) -> String {
    let mut out = String::new();
    let mut line = String::new();
    let mut depth = 0i32;
    let mut word = String::new();
    // A group in brackets or parentheses stays on one line.
    for c in usage.split_whitespace().collect::<Vec<_>>().join(" ").chars().chain([' ']) {
        depth += i32::from(c == '[' || c == '(') - i32::from(c == ']' || c == ')');
        if c == ' ' && depth == 0 {
            if line.len() + word.len() > 96 && !line.is_empty() {
                out.push_str(line.trim_end());
                out.push('\n');
                line = "    ".into();
            }
            line.push_str(&word);
            line.push(' ');
            word.clear();
        } else {
            word.push(c);
        }
    }
    out.push_str(line.trim_end());
    out.push_str(&format!(
        "\n\nEach option is in {CLI_DOC}, \"podrunner run\". A key of run.toml is an option with no --.\n"
    ));
    out
}

#[cfg(test)]
mod tests {
    use crate::cli::run;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    /// T-208: each command has its help, with no other argument.
    #[test]
    fn each_command_has_its_help() {
        for (a, want) in [
            (&["run", "--help"][..], "--runner-dir DIR"),
            (&["run", "-h"], "--runner-dir DIR"),
            (&["run", "w.yml", "-h"], "--runner-dir DIR"),
            (&["doctor", "--help"], "usage: podrunner doctor"),
            (&["status", "-h"], "usage: podrunner status"),
            (&["help", "stop"], "stop --wait"),
            (&["version"], "podrunner "),
        ] {
            let o = run(&args(a));
            assert_eq!(o.code, 0, "{a:?}: {}", o.stderr);
            assert!(o.stdout.contains(want), "{a:?}: {}", o.stdout);
        }
        assert_eq!(run(&args(&["help", "nope"])).code, 64);
    }
}
