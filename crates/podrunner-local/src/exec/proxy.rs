//! The proxy and the CA files of the caller in a unit with the network of
//! the host (T-284): on a host whose only way out is a proxy that
//! intercepts TLS, a job needs both. Each CA file goes into the unit as a
//! credential file, with no volume (a volume moves a podvm unit off the
//! namespace rung, T-261).

use std::collections::BTreeMap;

use podrunner_schema::{CredentialFile, Secret};

use super::host::PASSED;

/// The variables of `PASSED` that name a CA file.
const CA_VARS: &[&str] =
    &["SSL_CERT_FILE", "NODE_EXTRA_CA_CERTS", "GIT_SSL_CAINFO", "REQUESTS_CA_BUNDLE", "CURL_CA_BUNDLE"];

/// The variables and the CA files of a unit.
pub(crate) type UnitEnv = (Vec<(String, String)>, Vec<CredentialFile>);

/// The environment of a unit and its CA files, from the variables `vars` of
/// the caller; `dir` is where the unit sees its credential files. A `held`
/// unit takes the CA files only: its hold gives it its way out, and the
/// proxy of the host behind the hold can intercept TLS (T-320).
pub(crate) fn unit_env(dir: &str, vars: impl Iterator<Item = (String, String)>, held: bool) -> Result<UnitEnv, String> {
    let (env, files) = all_env(dir, vars)?;
    Ok(match held {
        true => (env.into_iter().filter(|(k, _)| CA_VARS.contains(&k.as_str())).collect(), files),
        false => (env, files),
    })
}

fn all_env(dir: &str, vars: impl Iterator<Item = (String, String)>) -> Result<UnitEnv, String> {
    let vars: BTreeMap<String, String> = vars.filter(|(_, v)| !v.is_empty()).collect();
    let mut env = Vec::new();
    let mut files: Vec<CredentialFile> = Vec::new();
    let mut by_path: BTreeMap<String, String> = BTreeMap::new();
    for k in PASSED.iter().filter(|k| **k != "PATH" && **k != "SSL_CERT_DIR") {
        let Some(v) = vars.get(*k) else { continue };
        if !CA_VARS.contains(k) {
            env.push((k.to_string(), v.clone()));
            continue;
        }
        let name = match by_path.get(v) {
            Some(n) => n.clone(),
            None => {
                let text = std::fs::read_to_string(v).map_err(|e| format!("{k}={v}: {e}"))?;
                let n = format!("podrunner-ca-{}.crt", by_path.len());
                files.push(CredentialFile { name: n.clone(), content: Secret::new(text) });
                by_path.insert(v.clone(), n.clone());
                n
            }
        };
        env.push((k.to_string(), format!("{dir}/{name}")));
    }
    let view: Vec<(&str, String)> = env.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    if let Some((k, v)) = super::host::node_proxy(&view) {
        env.push((k.to_string(), v));
    }
    if env.iter().any(|(k, _)| k.to_ascii_lowercase().ends_with("_proxy") && k != "NODE_USE_ENV_PROXY") {
        let mut no = vec!["127.0.0.1".to_string(), "localhost".to_string(), "::1".to_string()];
        for k in ["NO_PROXY", "no_proxy"] {
            let more = vars.get(k).into_iter().flat_map(|v| v.split(',')).map(str::trim);
            no.extend(more.filter(|v| !v.is_empty() && !v.chars().all(|c| c == ':')).map(String::from));
        }
        let mut seen = std::collections::BTreeSet::new();
        no.retain(|v| seen.insert(v.clone()));
        env.push(("NO_PROXY".into(), no.join(",")));
        env.push(("no_proxy".into(), no.join(",")));
    }
    Ok((env, files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_gets_the_proxy_and_each_ca_file_once() {
        let ca = std::env::temp_dir().join(format!("podrunner-proxy-ca-{}.crt", std::process::id()));
        std::fs::write(&ca, "-----BEGIN CERTIFICATE-----\nmade-up\n").unwrap();
        let p = ca.display().to_string();
        let vars = [
            ("HTTPS_PROXY", "http://127.0.0.1:3128"),
            ("SSL_CERT_FILE", p.as_str()),
            ("NODE_EXTRA_CA_CERTS", p.as_str()),
            ("PATH", "/x"),
            ("NO_PROXY", "::,intra.example"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()));
        let (env, files) = unit_env("/podrunner/.runner", vars.clone().into_iter(), false).unwrap();
        let get = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("HTTPS_PROXY"), Some("http://127.0.0.1:3128"));
        assert_eq!(get("SSL_CERT_FILE"), Some("/podrunner/.runner/podrunner-ca-0.crt"));
        assert_eq!(get("NODE_EXTRA_CA_CERTS"), Some("/podrunner/.runner/podrunner-ca-0.crt"));
        assert_eq!(get("PATH"), None, "the unit keeps the PATH of its image");
        assert_eq!(get("NO_PROXY"), Some("127.0.0.1,localhost,::1,intra.example"));
        assert_eq!(get("NODE_USE_ENV_PROXY"), Some("1"), "fetch of Node uses the proxy (T-299)");
        assert_eq!(files.len(), 1, "one file for one path");
        let (held, _) = unit_env("/podrunner/.runner", vars.into_iter(), true).unwrap();
        let held = |k: &str| held.iter().any(|(n, _)| n == k);
        assert!(
            held("NODE_EXTRA_CA_CERTS") && !held("HTTPS_PROXY") && !held("NO_PROXY"),
            "a held unit: the CA only (T-320)"
        );
        let (env, files) = unit_env("/d", std::iter::empty(), false).unwrap();
        assert!(env.is_empty() && files.is_empty(), "no proxy, nothing");
        let _ = std::fs::remove_file(&ca);
    }
}
