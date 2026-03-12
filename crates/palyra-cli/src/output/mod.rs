use anyhow::{Context, Result};
use serde::Serialize;

pub(crate) mod approvals;
pub(crate) mod channels;
pub(crate) mod skills;
pub(crate) mod support_bundle;

pub(crate) fn print_json_pretty<T>(value: &T, error_context: &'static str) -> Result<()>
where
    T: Serialize,
{
    println!("{}", serde_json::to_string_pretty(value).context(error_context)?);
    Ok(())
}
