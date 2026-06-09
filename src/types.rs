//! Digital advertising platform domain model.
//!
//! Broad ads platform: campaigns with budgets and objectives, ad groups with
//! bids and audiences, ads/creatives, daily performance metrics (impressions,
//! clicks, spend, conversions, revenue) with derived KPIs (CTR/CPC/CPA/ROAS),
//! budget pacing, and optimization suggestions. The Campaign Optimizer agent is
//! a client. Spend-affecting changes (budget, bid, campaign status) are gated.

use chrono::{DateTime, NaiveDate, Utc};
use rmcp::schemars;
use serde::{Deserialize, Serialize};

// ─── campaigns ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    Awareness,
    Traffic,
    Conversions,
    Sales,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    Draft,
    Active,
    Paused,
    Ended,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Campaign {
    pub id: String,
    pub name: String,
    pub channel: String, // search | social | display | video
    pub objective: Objective,
    pub status: CampaignStatus,
    /// Daily budget in account currency.
    pub daily_budget: f64,
    pub currency: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── ad groups ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BidStrategy {
    /// Manual cost-per-click.
    ManualCpc,
    /// Maximize conversions (automated).
    MaxConversions,
    /// Target cost-per-acquisition.
    TargetCpa,
    /// Target return on ad spend.
    TargetRoas,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdGroup {
    pub id: String,
    pub campaign_id: String,
    pub name: String,
    pub bid_strategy: BidStrategy,
    /// Bid amount (CPC) or target (CPA/ROAS) depending on strategy.
    pub bid: f64,
    pub audience_id: Option<String>,
    pub status: CampaignStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── ads / creatives ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Ad {
    pub id: String,
    pub ad_group_id: String,
    pub headline: String,
    pub body: String,
    pub final_url: String,
    pub status: CampaignStatus,
    pub created_at: DateTime<Utc>,
}

// ─── audiences ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Audience {
    pub id: String,
    pub name: String,
    /// e.g. "interest", "remarketing", "lookalike", "custom".
    pub kind: String,
    pub definition: String,
    pub estimated_size: u64,
    pub created_at: DateTime<Utc>,
}

// ─── metrics ─────────────────────────────────────────────────────────────────

/// A daily performance record at the ad-group level.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricRecord {
    pub ad_group_id: String,
    pub date: NaiveDate,
    pub impressions: u64,
    pub clicks: u64,
    pub spend: f64,
    pub conversions: u64,
    pub revenue: f64,
}

// ─── audit trail ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AuditEntry {
    pub at: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub detail: String,
}
