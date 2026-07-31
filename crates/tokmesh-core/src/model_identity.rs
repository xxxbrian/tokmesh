//! Client-aware model-id resolution for grouping and submit identity.
//!
//! Narrow suffix rules:
//! - OpenCode: `gpt-…-fast` → base model (e.g. `gpt-5.6-sol-fast` → `gpt-5.6-sol`).
//!   Client must be `opencode`; the model segment must start with `gpt-` and end
//!   with `-fast`.
//! - Any client: `grok-…-build` → base model (e.g. `grok-4.5-build` → `grok-4.5`).
//!   Provider/vendor path segments are preserved for later identity matching.

/// Apply suffix rules at a grouping, aggregation, or submit identity boundary.
/// Raw message model ids must remain unchanged until fallback pricing has run.
pub fn resolve_model_id(client: &str, raw_model_id: &str) -> String {
    // Provider-qualified ids still need suffix resolution, but only the final
    // model segment may be rewritten. The provider path remains identity data.
    let model_start = raw_model_id.rfind('/').map_or(0, |index| index + 1);
    let model_id = &raw_model_id[model_start..];
    let mut resolved = model_id;

    if let Some(base) = strip_grok_build_suffix(resolved) {
        resolved = base;
    }

    if client.eq_ignore_ascii_case("opencode") {
        if let Some(base) = strip_opencode_gpt_fast_suffix(resolved) {
            resolved = base;
        }
    }

    if resolved.len() == model_id.len() {
        raw_model_id.to_string()
    } else {
        format!("{}{resolved}", &raw_model_id[..model_start])
    }
}

/// `grok-4.5-build` → `grok-4.5`. Only trailing `-build` on a `grok-` id.
fn strip_grok_build_suffix(model_id: &str) -> Option<&str> {
    let lower = model_id.to_ascii_lowercase();
    if !lower.starts_with("grok-") {
        return None;
    }
    let base = lower.strip_suffix("-build")?;
    if base.is_empty() || base == "grok" {
        return None;
    }
    Some(&model_id[..base.len()])
}

/// `gpt-5.6-sol-fast` → `gpt-5.6-sol`. Requires `gpt-` prefix and `-fast` suffix.
fn strip_opencode_gpt_fast_suffix(model_id: &str) -> Option<&str> {
    let lower = model_id.to_ascii_lowercase();
    if !lower.starts_with("gpt-") {
        return None;
    }
    let base = lower.strip_suffix("-fast")?;
    // Need a non-empty stem after `gpt-` (rejects `gpt-fast` and bare `gpt-`).
    if base.len() <= "gpt-".len() {
        return None;
    }
    Some(&model_id[..base.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_gpt_fast_strips_to_base() {
        assert_eq!(
            resolve_model_id("opencode", "gpt-5.6-sol-fast"),
            "gpt-5.6-sol"
        );
        assert_eq!(
            resolve_model_id("opencode", "openai/gpt-5.6-sol-fast"),
            "openai/gpt-5.6-sol"
        );
    }

    #[test]
    fn opencode_non_gpt_fast_unchanged() {
        assert_eq!(
            resolve_model_id("opencode", "claude-opus-4-6-fast"),
            "claude-opus-4-6-fast"
        );
    }

    #[test]
    fn non_opencode_gpt_fast_unchanged() {
        assert_eq!(
            resolve_model_id("codex", "gpt-5.6-sol-fast"),
            "gpt-5.6-sol-fast"
        );
    }

    #[test]
    fn grok_build_strips_to_base() {
        assert_eq!(resolve_model_id("grok", "grok-4.5-build"), "grok-4.5");
        // Same rule is global for any client that records a grok-*-build id.
        assert_eq!(resolve_model_id("opencode", "grok-4.5-build"), "grok-4.5");
        assert_eq!(
            resolve_model_id("grok", "xai/grok-4.5-build"),
            "xai/grok-4.5"
        );
    }

    #[test]
    fn non_grok_build_suffix_kept() {
        assert_eq!(
            resolve_model_id("opencode", "my-tool-build"),
            "my-tool-build"
        );
    }

    #[test]
    fn suffix_resolution_does_not_rewrite_vendor_or_provider_segments() {
        assert_eq!(
            resolve_model_id("opencode", "vendor-fast/openai-fast/gpt-5.6-sol-fast"),
            "vendor-fast/openai-fast/gpt-5.6-sol"
        );
        assert_eq!(
            resolve_model_id("grok", "vendor-build/xai-build/grok-4.5-build"),
            "vendor-build/xai-build/grok-4.5"
        );
    }
}
