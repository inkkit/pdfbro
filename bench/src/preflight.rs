use std::process::Command;
use anyhow::Context;

pub struct ToolVersions {
    pub chrome: String,
    pub libreoffice: String,
}

pub fn get_versions(container_name: &str) -> anyhow::Result<ToolVersions> {
    let chrome = exec_in_container(container_name, &["chromium", "--version"])
        .or_else(|_| exec_in_container(container_name, &["google-chrome", "--version"]))
        .or_else(|_| exec_in_container(container_name, &["chromium-browser", "--version"]))
        .context("could not determine Chrome version")?;

    let libreoffice = exec_in_container(container_name, &["soffice", "--version"])
        .context("could not determine LibreOffice version")?;

    Ok(ToolVersions {
        chrome: chrome.trim().to_string(),
        libreoffice: libreoffice.trim().to_string(),
    })
}

fn exec_in_container(container: &str, cmd: &[&str]) -> anyhow::Result<String> {
    let mut args = vec!["exec", container];
    args.extend_from_slice(cmd);
    let output = Command::new("docker").args(&args).output()?;
    if !output.status.success() {
        anyhow::bail!("command failed: {:?}", cmd);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn check(folio_container: &str, gotenberg_container: &str) -> anyhow::Result<()> {
    let folio = get_versions(folio_container)?;
    let gotenberg = get_versions(gotenberg_container)?;

    println!("Folio     — Chrome: {}  LibreOffice: {}", folio.chrome, folio.libreoffice);
    println!("Gotenberg — Chrome: {}  LibreOffice: {}", gotenberg.chrome, gotenberg.libreoffice);

    let folio_major = major_version(&folio.chrome);
    let gotenberg_major = major_version(&gotenberg.chrome);
    if folio_major != gotenberg_major {
        anyhow::bail!(
            "Chrome version mismatch: Folio={} Gotenberg={}. Aborting.",
            folio.chrome, gotenberg.chrome
        );
    }

    Ok(())
}

fn major_version(s: &str) -> String {
    s.split_whitespace()
        .find(|p| p.chars().next().map_or(false, |c| c.is_ascii_digit()))
        .and_then(|v| v.split('.').next())
        .unwrap_or("")
        .to_string()
}
