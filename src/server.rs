//! MCP tool surface for the ads platform.
//!
//! Reads (entities, performance, pacing, optimize) are `read_only`. Structural
//! writes (create campaign/ad group/ad/audience, ingest metrics) are
//! `internal_write`. Three changes directly affect spend and are gated
//! (`requires_approval`, `external_write`): `set_budget`, `set_bid`, and
//! `set_campaign_status`.

use crate::store::AdsStore;
use crate::types::*;
use adk_mcp_sdk::{HealthCheck, HealthStatus};
use chrono::NaiveDate;
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use std::sync::Arc;

fn dactor() -> String { "agent".into() }
fn date(s: &Option<String>) -> Option<NaiveDate> { s.as_ref().and_then(|x| NaiveDate::parse_from_str(x, "%Y-%m-%d").ok()) }
fn today() -> NaiveDate { chrono::Utc::now().date_naive() }
fn dusd() -> String { "USD".into() }
fn dlimit() -> usize { 50 }

// ─── inputs ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateCampaignInput {
    pub name: String,
    #[serde(default = "dsearch")] pub channel: String,
    #[serde(default = "dconv")] pub objective: Objective,
    #[serde(default)] pub daily_budget: f64,
    #[serde(default = "dusd")] pub currency: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[serde(default = "dactor")] pub actor: String,
}
fn dsearch() -> String { "search".into() }
fn dconv() -> Objective { Objective::Conversions }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CampaignIdInput { pub campaign_id: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListCampaignsInput { pub status: Option<CampaignStatus>, pub channel: Option<String> }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetStatusInput { pub campaign_id: String, pub status: CampaignStatus, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetBudgetInput { pub campaign_id: String, pub daily_budget: f64, #[serde(default = "dactor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateAdGroupInput { pub campaign_id: String, pub name: String, #[serde(default = "dcpc")] pub bid_strategy: BidStrategy, #[serde(default)] pub bid: f64, pub audience_id: Option<String>, #[serde(default = "dactor")] pub actor: String }
fn dcpc() -> BidStrategy { BidStrategy::ManualCpc }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AdGroupIdInput { pub ad_group_id: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetBidInput { pub ad_group_id: String, pub bid: f64, #[serde(default = "dactor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateAdInput { pub ad_group_id: String, pub headline: String, #[serde(default)] pub body: String, #[serde(default)] pub final_url: String, #[serde(default = "dactor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateAudienceInput { pub name: String, #[serde(default = "dint")] pub kind: String, #[serde(default)] pub definition: String, #[serde(default)] pub estimated_size: u64, #[serde(default = "dactor")] pub actor: String }
fn dint() -> String { "interest".into() }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EmptyInput {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IngestMetricsInput {
    pub ad_group_id: String,
    pub date: Option<String>,
    #[serde(default)] pub impressions: u64,
    #[serde(default)] pub clicks: u64,
    #[serde(default)] pub spend: f64,
    #[serde(default)] pub conversions: u64,
    #[serde(default)] pub revenue: f64,
    #[serde(default = "dactor")] pub actor: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AuditLogInput { #[serde(default = "dlimit")] pub limit: usize }

// ─── server ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AdsServer { pub store: Arc<AdsStore> }

#[tool_router]
impl AdsServer {
    // campaigns
    #[tool(description = "Create a campaign (starts in draft). objective: awareness/traffic/conversions/sales; channel: search/social/display/video.")]
    fn create_campaign(&self, Parameters(i): Parameters<CreateCampaignInput>) -> String {
        let start = date(&i.start_date).unwrap_or_else(today);
        let c = self.store.create_campaign(&i.name, &i.channel, i.objective, i.daily_budget, &i.currency, start, date(&i.end_date), &i.actor);
        serde_json::to_string_pretty(&c).unwrap()
    }

    #[tool(description = "Get a campaign by id.")]
    fn get_campaign(&self, Parameters(i): Parameters<CampaignIdInput>) -> String {
        match self.store.get_campaign(&i.campaign_id) {
            Some(c) => serde_json::to_string_pretty(&c).unwrap(), None => format!("Campaign not found: {}", i.campaign_id) }
    }

    #[tool(description = "List campaigns, optionally by status and/or channel.")]
    fn list_campaigns(&self, Parameters(i): Parameters<ListCampaignsInput>) -> String {
        let v = self.store.list_campaigns(i.status, i.channel.as_deref());
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "campaigns": v})).unwrap()
    }

    #[tool(description = "Set a campaign's status (draft/active/paused/ended). Affects whether it spends — gated.")]
    fn set_campaign_status(&self, Parameters(i): Parameters<SetStatusInput>) -> String {
        match self.store.set_campaign_status(&i.campaign_id, i.status, &i.actor) {
            Ok(c) => serde_json::to_string_pretty(&c).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Set a campaign's daily budget. Directly affects spend — gated.")]
    fn set_budget(&self, Parameters(i): Parameters<SetBudgetInput>) -> String {
        match self.store.set_budget(&i.campaign_id, i.daily_budget, &i.actor) {
            Ok(c) => serde_json::to_string_pretty(&c).unwrap(), Err(e) => format!("Error: {e}") }
    }

    // ad groups
    #[tool(description = "Create an ad group under a campaign. bid_strategy: manual_cpc/max_conversions/target_cpa/target_roas; bid is the CPC or target value.")]
    fn create_ad_group(&self, Parameters(i): Parameters<CreateAdGroupInput>) -> String {
        match self.store.create_ad_group(&i.campaign_id, &i.name, i.bid_strategy, i.bid, i.audience_id, &i.actor) {
            Ok(g) => serde_json::to_string_pretty(&g).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Get an ad group by id.")]
    fn get_ad_group(&self, Parameters(i): Parameters<AdGroupIdInput>) -> String {
        match self.store.get_ad_group(&i.ad_group_id) {
            Some(g) => serde_json::to_string_pretty(&g).unwrap(), None => format!("Ad group not found: {}", i.ad_group_id) }
    }

    #[tool(description = "List ad groups in a campaign.")]
    fn list_ad_groups(&self, Parameters(i): Parameters<CampaignIdInput>) -> String {
        let v = self.store.ad_groups_for(&i.campaign_id);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "ad_groups": v})).unwrap()
    }

    #[tool(description = "Set an ad group's bid or target (CPC/CPA/ROAS depending on strategy). Directly affects spend — gated.")]
    fn set_bid(&self, Parameters(i): Parameters<SetBidInput>) -> String {
        match self.store.set_bid(&i.ad_group_id, i.bid, &i.actor) {
            Ok(g) => serde_json::to_string_pretty(&g).unwrap(), Err(e) => format!("Error: {e}") }
    }

    // ads
    #[tool(description = "Create an ad/creative under an ad group.")]
    fn create_ad(&self, Parameters(i): Parameters<CreateAdInput>) -> String {
        match self.store.create_ad(&i.ad_group_id, &i.headline, &i.body, &i.final_url, &i.actor) {
            Ok(a) => serde_json::to_string_pretty(&a).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "List ads in an ad group.")]
    fn list_ads(&self, Parameters(i): Parameters<AdGroupIdInput>) -> String {
        let v = self.store.ads_for(&i.ad_group_id);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "ads": v})).unwrap()
    }

    // audiences
    #[tool(description = "Create an audience (interest/remarketing/lookalike/custom).")]
    fn create_audience(&self, Parameters(i): Parameters<CreateAudienceInput>) -> String {
        let a = self.store.create_audience(&i.name, &i.kind, &i.definition, i.estimated_size, &i.actor);
        serde_json::to_string_pretty(&a).unwrap()
    }

    #[tool(description = "List audiences.")]
    fn list_audiences(&self, Parameters(_): Parameters<EmptyInput>) -> String {
        let v = self.store.list_audiences();
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "audiences": v})).unwrap()
    }

    // metrics & performance
    #[tool(description = "Ingest a daily performance record for an ad group (impressions/clicks/spend/conversions/revenue). Replaces any same-date record.")]
    fn ingest_metrics(&self, Parameters(i): Parameters<IngestMetricsInput>) -> String {
        let d = date(&i.date).unwrap_or_else(today);
        match self.store.ingest_metrics(&i.ad_group_id, d, i.impressions, i.clicks, i.spend, i.conversions, i.revenue, &i.actor) {
            Ok(r) => serde_json::to_string_pretty(&r).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Ad-group performance with derived KPIs: CTR, CPC, CPA, CVR, ROAS.")]
    fn ad_group_performance(&self, Parameters(i): Parameters<AdGroupIdInput>) -> String {
        match self.store.ad_group_performance(&i.ad_group_id) {
            Some(v) => serde_json::to_string_pretty(&v).unwrap(), None => format!("Ad group not found: {}", i.ad_group_id) }
    }

    #[tool(description = "Campaign performance rolled up across ad groups (CTR/CPC/CPA/ROAS).")]
    fn campaign_performance(&self, Parameters(i): Parameters<CampaignIdInput>) -> String {
        match self.store.campaign_performance(&i.campaign_id) {
            Some(v) => serde_json::to_string_pretty(&v).unwrap(), None => format!("Campaign not found: {}", i.campaign_id) }
    }

    #[tool(description = "Budget pacing for a campaign: today's spend vs daily budget, with a pacing flag.")]
    fn budget_pacing(&self, Parameters(i): Parameters<CampaignIdInput>) -> String {
        match self.store.budget_pacing(&i.campaign_id) {
            Some(v) => serde_json::to_string_pretty(&v).unwrap(), None => format!("Campaign not found: {}", i.campaign_id) }
    }

    #[tool(description = "Optimization suggestions for a campaign's ad groups (scale winners, cut losers, fix low CTR / CPA over target). Powers the Campaign Optimizer; recommendations only — spend changes stay gated.")]
    fn optimize(&self, Parameters(i): Parameters<CampaignIdInput>) -> String {
        match self.store.optimize(&i.campaign_id) {
            Some(v) => serde_json::to_string_pretty(&v).unwrap(), None => format!("Campaign not found: {}", i.campaign_id) }
    }

    #[tool(description = "Recent audit-trail entries (most recent first).")]
    fn audit_log(&self, Parameters(i): Parameters<AuditLogInput>) -> String {
        let v = self.store.audit_log(i.limit);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "entries": v})).unwrap()
    }
}

#[async_trait::async_trait]
impl HealthCheck for AdsServer {
    async fn check_health(&self) -> HealthStatus {
        HealthStatus { healthy: true, message: Some("operational".into()), latency_ms: Some(1) }
    }
}

adk_mcp_sdk::mcp_2026_server! {
    server: AdsServer,
    task_tools: [],
    approval_tools: ["set_campaign_status", "set_budget", "set_bid"],
    cache_ttl_ms: 60_000,
}
