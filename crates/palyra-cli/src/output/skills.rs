use crate::*;

pub(crate) fn emit_status(
    event_kind: &str,
    response: &SkillStatusResponse,
    json_output: bool,
) -> Result<()> {
    if json_output {
        return super::print_json_pretty(response, "failed to encode skill status payload as JSON");
    }

    println!(
        "{event_kind} skill_id={} version={} status={} reason={} detected_at_ms={} operator_principal={}",
        response.skill_id,
        response.version,
        response.status,
        response.reason.clone().unwrap_or_else(|| "none".to_owned()),
        response.detected_at_ms,
        response.operator_principal,
    );
    Ok(())
}
