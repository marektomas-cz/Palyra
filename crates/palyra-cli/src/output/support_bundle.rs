use crate::*;

pub(crate) fn emit_export(
    output_path: &Path,
    encoded_bytes: usize,
    bundle: &SupportBundle,
) -> Result<()> {
    println!(
        "support_bundle.export path={} bytes={} truncated={} warnings={}",
        output_path.display(),
        encoded_bytes,
        bundle.truncated,
        bundle.warnings.len()
    );
    std::io::stdout().flush().context("stdout flush failed")
}
