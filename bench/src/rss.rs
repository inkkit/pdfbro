use std::process::Command;

pub fn sample_rss_mib(container_name: &str) -> Option<u64> {
    let output = Command::new("docker")
        .args(["stats", "--no-stream", "--format", "{{.Name}}\t{{.MemUsage}}", container_name])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() == 2 && parts[0].contains(container_name) {
            return parse_mib(parts[1].split('/').next()?.trim());
        }
    }
    None
}

fn parse_mib(s: &str) -> Option<u64> {
    if let Some(v) = s.strip_suffix("MiB") {
        return v.trim().parse::<f64>().ok().map(|f| f as u64);
    }
    if let Some(v) = s.strip_suffix("GiB") {
        return v.trim().parse::<f64>().ok().map(|f| (f * 1024.0) as u64);
    }
    if let Some(v) = s.strip_suffix("kB") {
        return v.trim().parse::<f64>().ok().map(|f| (f / 1024.0) as u64);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mib_formats() {
        assert_eq!(parse_mib("512MiB"), Some(512));
        assert_eq!(parse_mib("1.5GiB"), Some(1536));
        assert_eq!(parse_mib("512kB"), Some(0));
    }
}
