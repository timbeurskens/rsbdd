//! Downloads files from the TPTP problems and solutions directory, which serves every file as a gzipped tar archive.

use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, bail};
use flate2::read::GzDecoder;

const BASE_URL: &str = "https://seetptp.tptp.org/download/TPTPWorld/TPTP/";

/// The path of a problem relative to the TPTP directory, such as `Problems/PUZ/PUZ001-1.p` for `PUZ001-1`
pub fn problem_path(name: &str) -> anyhow::Result<String> {
    let name = name.strip_suffix(".p").unwrap_or(name);
    let domain = name
        .get(..3)
        .filter(|d| d.chars().all(|c| c.is_ascii_uppercase()));
    match domain {
        Some(domain) if name.len() > 3 => Ok(format!("Problems/{domain}/{name}.p")),
        _ => bail!("'{name}' is not a TPTP problem name, such as PUZ001-1"),
    }
}

/// The agent for all downloads, which reuses connections. Redirects are not followed, as a file that does not
/// exist redirects to the home page.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .max_redirects(0)
            .build()
            .into()
    })
}

/// Download a file, given its path relative to the TPTP directory, such as `Axioms/SET001-0.ax`
pub fn fetch(path: &str) -> anyhow::Result<String> {
    let url = format!("{BASE_URL}{path}");
    let response = match agent().get(&url).call() {
        Ok(response) if response.status().is_redirection() => {
            bail!("{path} is not in the TPTP library")
        }
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => bail!("{path} is not in the TPTP library"),
        Err(ureq::Error::StatusCode(429)) => {
            bail!(
                "the TPTP server limits the number of downloads, try again later or use a local copy with --tptp-dir"
            )
        }
        Err(error) => return Err(error).with_context(|| format!("downloading {url}")),
    };

    let file_name = Path::new(path).file_name();
    let mut archive = tar::Archive::new(GzDecoder::new(response.into_body().into_reader()));
    for entry in archive
        .entries()
        .with_context(|| format!("reading the archive of {url}"))?
    {
        let mut entry = entry?;
        if entry.path()?.file_name() == file_name {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            return Ok(contents);
        }
    }
    bail!("the archive of {url} does not contain {path}")
}
