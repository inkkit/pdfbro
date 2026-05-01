//! `folio metadata read` / `write`.

use anyhow::{Context, anyhow};
use engine::pdfops::{self, Metadata};

use crate::args::{MetadataAction, MetadataReadArgs, MetadataWriteArgs};
use crate::exit::UsageError;
use crate::io_helpers::{read_input_sync, write_output};

/// Top-level dispatcher for `metadata read | write`.
pub(crate) fn run(action: &MetadataAction) -> anyhow::Result<()> {
    match action {
        MetadataAction::Read(a) => run_read(a),
        MetadataAction::Write(a) => run_write(a),
    }
}

fn run_read(args: &MetadataReadArgs) -> anyhow::Result<()> {
    let pdf = read_input_sync(&args.input)?;
    let meta = pdfops::read_metadata(&pdf).context("reading metadata")?;
    let json = serde_json::to_string_pretty(&meta).context("serializing metadata")?;
    println!("{json}");
    Ok(())
}

fn run_write(args: &MetadataWriteArgs) -> anyhow::Result<()> {
    let pdf = read_input_sync(&args.input)?;

    let mut meta = if let Some(path) = &args.from_json {
        let body =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str::<Metadata>(&body)
            .map_err(|e| anyhow!("invalid metadata JSON in {}: {e}", path.display()))?
    } else {
        Metadata::default()
    };

    for (key, value) in &args.set {
        apply_set(&mut meta, key, value)?;
    }

    let out = pdfops::write_metadata(&pdf, &meta).context("writing metadata")?;
    write_output(&args.output, &out)
}

/// Apply a single `--set KEY=VALUE` pair onto `meta`. Standard keys map
/// to typed fields; everything else lands in `Metadata::custom`. An
/// empty value clears the corresponding field (matching engine
/// `write_metadata` semantics).
fn apply_set(meta: &mut Metadata, key: &str, value: &str) -> anyhow::Result<()> {
    let v = value.to_string();
    match key {
        "Title" => meta.title = Some(v),
        "Author" => meta.author = Some(v),
        "Subject" => meta.subject = Some(v),
        "Keywords" => meta.keywords = Some(v.split(", ").map(|s| s.to_string()).collect()),
        "Creator" => meta.creator = Some(v),
        "Producer" => meta.producer = Some(v),
        "CreationDate" => meta.creation_date = Some(v),
        "ModDate" => meta.mod_date = Some(v),
        "" => {
            return Err(anyhow!("--set: empty key").context(UsageError));
        }
        other => {
            meta.custom.insert(other.to_string(), serde_json::Value::String(v));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_set_title() {
        let mut m = Metadata::default();
        apply_set(&mut m, "Title", "Hello").unwrap();
        assert_eq!(m.title.as_deref(), Some("Hello"));
    }

    #[test]
    fn apply_set_empty_value_clears_via_engine_semantics() {
        let mut m = Metadata {
            title: Some("Old".into()),
            ..Default::default()
        };
        apply_set(&mut m, "Title", "").unwrap();
        // Empty string is preserved in the struct; engine's write_metadata
        // treats Some("") as "delete".
        assert_eq!(m.title.as_deref(), Some(""));
    }

    #[test]
    fn apply_set_unknown_key_lands_in_custom() {
        let mut m = Metadata::default();
        apply_set(&mut m, "Foo", "bar").unwrap();
        assert_eq!(m.custom.get("Foo").and_then(|v| v.as_str()), Some("bar"));
    }
}
