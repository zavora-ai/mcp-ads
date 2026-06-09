//! Validate mcp-server.toml parses, passes SDK validation, has the right tool
//! count, and gates the spend-affecting writes.

use adk_mcp_sdk::manifest::ServerManifest;
use std::path::Path;

fn manifest() -> ServerManifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("mcp-server.toml");
    ServerManifest::from_file(&path).expect("manifest should parse")
}

#[test]
fn manifest_parses_and_validates() {
    let m = manifest();
    assert!(m.validate().is_empty(), "validation errors: {:?}", m.validate());
    assert_eq!(m.server_id, "mcp_ads");
    assert_eq!(m.domain, "sales-marketing");
    assert_eq!(m.tools.len(), 19, "expected 19 declared tools");
}

#[test]
fn spend_writes_are_gated_external() {
    use adk_mcp_sdk::risk::RiskClass;
    let m = manifest();
    for name in ["set_budget", "set_bid", "set_campaign_status"] {
        let t = m.tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("{name} present"));
        assert!(t.requires_approval, "{name} must require approval");
        assert_eq!(t.risk_class, RiskClass::ExternalWrite, "{name} must be external_write");
    }
}

#[test]
fn optimize_and_reads_are_read_only() {
    use adk_mcp_sdk::risk::RiskClass;
    let m = manifest();
    for name in ["get_campaign", "list_campaigns", "ad_group_performance", "campaign_performance", "budget_pacing", "optimize", "list_audiences", "audit_log"] {
        let t = m.tools.iter().find(|t| t.name == name).unwrap();
        assert_eq!(t.risk_class, RiskClass::ReadOnly, "{name} should be read_only");
    }
}

#[test]
fn only_spend_writes_gated() {
    let m = manifest();
    let gated: Vec<&str> = m.tools.iter().filter(|t| t.requires_approval).map(|t| t.name.as_str()).collect();
    assert_eq!(gated, vec!["set_campaign_status", "set_budget", "set_bid"]);
}
