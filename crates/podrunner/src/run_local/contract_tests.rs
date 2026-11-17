//! The command line of `run` as a contract (T-245): each flag that the
//! usage names is a flag of the parser, each flag of the parser is in the
//! usage, and `docs/cli.md` names the same flags as the usage.

use std::collections::BTreeSet;

use super::*;
use crate::run_local::USAGE;

/// The flags of a text: each `--NAME`.
fn flags(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i + 2 < b.len() {
        if &b[i..i + 2] == b"--" && b[i + 2].is_ascii_lowercase() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let end = (i + 2..b.len()).find(|&j| !(b[j].is_ascii_lowercase() || b[j] == b'-')).unwrap_or(b.len());
            out.insert(text[i..end].trim_end_matches('-').to_string());
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// The flags that `run_config::expand` and `watch::repeat` take before
/// the parser.
const BEFORE_THE_PARSER: [&str; 3] = ["--no-config", "--watch", "--schedule"];

/// The long forms of a short flag of the usage.
const LONG_FORMS: [&str; 1] = ["--quiet"];

#[test]
fn each_flag_of_the_usage_is_a_flag_of_the_parser() {
    for f in flags(USAGE) {
        if BEFORE_THE_PARSER.contains(&f.as_str()) {
            continue;
        }
        let e = parse(&["w.yml".to_string(), f.clone()]).err().unwrap_or_default();
        assert!(!e.starts_with("an unknown option"), "{f}: {e}");
    }
}

#[test]
fn each_flag_of_the_parser_is_in_the_usage() {
    let usage = flags(USAGE);
    // Each arm of the parser: `"--NAME" =>` or `"-x" | "--NAME" =>`.
    let source = include_str!("args.rs");
    for line in source.lines().filter(|l| l.contains("\" =>")) {
        for f in flags(line) {
            assert!(
                usage.contains(&f) || LONG_FORMS.contains(&f.as_str()),
                "{f} is a flag of the parser, not of the usage"
            );
        }
    }
}

#[test]
fn the_documents_name_the_flags_of_the_usage() {
    let cli = include_str!("../../../../docs/cli.md");
    let row = cli.lines().find(|l| l.starts_with("| `podrunner run (FILE")).expect("the row of run in docs/cli.md");
    let synopsis = row.split("` |").next().unwrap_or(row);
    assert_eq!(flags(synopsis), flags(USAGE), "docs/cli.md and the usage of run differ");
}
