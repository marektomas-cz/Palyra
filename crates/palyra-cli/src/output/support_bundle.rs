use crate::*;

pub(crate) fn emit_export(
    output_path: &Path,
    encoded_bytes: usize,
    bundle: &SupportBundle,
) -> Result<()> {
    if output::preferred_json(false) {
        return output::print_json_pretty(
            &json!({
                "path": output_path.display().to_string(),
                "bytes": encoded_bytes,
                "truncated": bundle.truncated,
                "warnings": bundle.warnings,
            }),
            "failed to encode support bundle export as JSON",
        );
    }
    println!(
        "support_bundle.export path={} bytes={} truncated={} warnings={}",
        output_path.display(),
        encoded_bytes,
        bundle.truncated,
        bundle.warnings.len()
    );
    std::io::stdout().flush().context("stdout flush failed")
}
