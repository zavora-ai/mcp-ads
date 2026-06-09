//! In-memory ads store with seeded data and engines.
//!
//! Thread-safe via per-collection `Mutex`. IDs come from a monotonic sequence
//! (`PREFIX-{n}` from 1000). Every spend-affecting change appends to an audit
//! trail. Engines: metrics ingest + derived KPIs (CTR/CPC/CPA/ROAS), budget
//! pacing, and rule-based optimization suggestions.

use crate::types::*;
use chrono::{Duration, NaiveDate, Utc};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct AdsStore {
    campaigns: Mutex<HashMap<String, Campaign>>,
    ad_groups: Mutex<HashMap<String, AdGroup>>,
    ads: Mutex<HashMap<String, Ad>>,
    audiences: Mutex<HashMap<String, Audience>>,
    metrics: Mutex<Vec<MetricRecord>>,
    audit_log: Mutex<Vec<AuditEntry>>,
    seq: Mutex<u64>,
}

impl Default for AdsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AdsStore {
    pub fn new() -> Self {
        let s = AdsStore {
            campaigns: Mutex::new(HashMap::new()),
            ad_groups: Mutex::new(HashMap::new()),
            ads: Mutex::new(HashMap::new()),
            audiences: Mutex::new(HashMap::new()),
            metrics: Mutex::new(Vec::new()),
            audit_log: Mutex::new(Vec::new()),
            seq: Mutex::new(1000),
        };
        s.seed();
        s
    }

    fn next(&self, prefix: &str) -> String {
        let mut n = self.seq.lock().unwrap();
        *n += 1;
        format!("{prefix}-{n}")
    }

    fn audit(&self, actor: &str, action: &str, detail: impl Into<String>) {
        self.audit_log.lock().unwrap().push(AuditEntry { at: Utc::now(), actor: actor.to_string(), action: action.to_string(), detail: detail.into() });
    }

    pub fn campaign_exists(&self, id: &str) -> bool { self.campaigns.lock().unwrap().contains_key(id) }

    // ─── campaigns ───────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn create_campaign(&self, name: &str, channel: &str, objective: Objective, daily_budget: f64, currency: &str, start_date: NaiveDate, end_date: Option<NaiveDate>, actor: &str) -> Campaign {
        let now = Utc::now();
        let c = Campaign {
            id: self.next("CMP"),
            name: name.to_string(),
            channel: channel.to_string(),
            objective,
            status: CampaignStatus::Draft,
            daily_budget,
            currency: currency.to_string(),
            start_date,
            end_date,
            created_at: now,
            updated_at: now,
        };
        self.campaigns.lock().unwrap().insert(c.id.clone(), c.clone());
        self.audit(actor, "create_campaign", c.id.clone());
        c
    }

    pub fn get_campaign(&self, id: &str) -> Option<Campaign> {
        self.campaigns.lock().unwrap().get(id).cloned()
    }

    pub fn list_campaigns(&self, status: Option<CampaignStatus>, channel: Option<&str>) -> Vec<Campaign> {
        let mut v: Vec<Campaign> = self.campaigns.lock().unwrap().values()
            .filter(|c| status.is_none_or(|s| c.status == s))
            .filter(|c| channel.is_none_or(|ch| c.channel.eq_ignore_ascii_case(ch)))
            .cloned().collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    /// Set a campaign's status (draft/active/paused/ended) — gated (spend).
    pub fn set_campaign_status(&self, id: &str, status: CampaignStatus, actor: &str) -> Result<Campaign, String> {
        let mut camps = self.campaigns.lock().unwrap();
        let c = camps.get_mut(id).ok_or_else(|| format!("Campaign not found: {id}"))?;
        if c.status == CampaignStatus::Ended { return Err("campaign is ended; cannot change status".into()); }
        c.status = status;
        c.updated_at = Utc::now();
        let out = c.clone();
        drop(camps);
        self.audit(actor, "set_campaign_status", format!("{id} -> {status:?}"));
        Ok(out)
    }

    /// Set a campaign's daily budget — gated (spend). Refuses negative.
    pub fn set_budget(&self, id: &str, daily_budget: f64, actor: &str) -> Result<Campaign, String> {
        if daily_budget < 0.0 { return Err("budget must be non-negative".into()); }
        let mut camps = self.campaigns.lock().unwrap();
        let c = camps.get_mut(id).ok_or_else(|| format!("Campaign not found: {id}"))?;
        let old = c.daily_budget;
        c.daily_budget = daily_budget;
        c.updated_at = Utc::now();
        let out = c.clone();
        drop(camps);
        self.audit(actor, "set_budget", format!("{id} {old} -> {daily_budget}"));
        Ok(out)
    }

    // ─── ad groups ───────────────────────────────────────────────────────

    pub fn create_ad_group(&self, campaign_id: &str, name: &str, bid_strategy: BidStrategy, bid: f64, audience_id: Option<String>, actor: &str) -> Result<AdGroup, String> {
        if !self.campaign_exists(campaign_id) { return Err(format!("Campaign not found: {campaign_id}")); }
        let now = Utc::now();
        let g = AdGroup { id: self.next("ADG"), campaign_id: campaign_id.to_string(), name: name.to_string(), bid_strategy, bid, audience_id, status: CampaignStatus::Active, created_at: now, updated_at: now };
        self.ad_groups.lock().unwrap().insert(g.id.clone(), g.clone());
        self.audit(actor, "create_ad_group", g.id.clone());
        Ok(g)
    }

    pub fn get_ad_group(&self, id: &str) -> Option<AdGroup> {
        self.ad_groups.lock().unwrap().get(id).cloned()
    }

    pub fn ad_groups_for(&self, campaign_id: &str) -> Vec<AdGroup> {
        let mut v: Vec<AdGroup> = self.ad_groups.lock().unwrap().values().filter(|g| g.campaign_id == campaign_id).cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Set an ad group's bid/target — gated (spend).
    pub fn set_bid(&self, ad_group_id: &str, bid: f64, actor: &str) -> Result<AdGroup, String> {
        if bid < 0.0 { return Err("bid must be non-negative".into()); }
        let mut groups = self.ad_groups.lock().unwrap();
        let g = groups.get_mut(ad_group_id).ok_or_else(|| format!("Ad group not found: {ad_group_id}"))?;
        let old = g.bid;
        g.bid = bid;
        g.updated_at = Utc::now();
        let out = g.clone();
        drop(groups);
        self.audit(actor, "set_bid", format!("{ad_group_id} {old} -> {bid}"));
        Ok(out)
    }

    // ─── ads ─────────────────────────────────────────────────────────────

    pub fn create_ad(&self, ad_group_id: &str, headline: &str, body: &str, final_url: &str, actor: &str) -> Result<Ad, String> {
        if self.get_ad_group(ad_group_id).is_none() { return Err(format!("Ad group not found: {ad_group_id}")); }
        let a = Ad { id: self.next("AD"), ad_group_id: ad_group_id.to_string(), headline: headline.to_string(), body: body.to_string(), final_url: final_url.to_string(), status: CampaignStatus::Active, created_at: Utc::now() };
        self.ads.lock().unwrap().insert(a.id.clone(), a.clone());
        self.audit(actor, "create_ad", a.id.clone());
        Ok(a)
    }

    pub fn ads_for(&self, ad_group_id: &str) -> Vec<Ad> {
        self.ads.lock().unwrap().values().filter(|a| a.ad_group_id == ad_group_id).cloned().collect()
    }

    // ─── audiences ───────────────────────────────────────────────────────

    pub fn create_audience(&self, name: &str, kind: &str, definition: &str, estimated_size: u64, actor: &str) -> Audience {
        let a = Audience { id: self.next("AUD"), name: name.to_string(), kind: kind.to_string(), definition: definition.to_string(), estimated_size, created_at: Utc::now() };
        self.audiences.lock().unwrap().insert(a.id.clone(), a.clone());
        self.audit(actor, "create_audience", a.id.clone());
        a
    }

    pub fn list_audiences(&self) -> Vec<Audience> {
        let mut v: Vec<Audience> = self.audiences.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    // ─── metrics & KPIs ──────────────────────────────────────────────────

    /// Ingest a daily metric record for an ad group (replaces same-date row).
    pub fn ingest_metrics(&self, ad_group_id: &str, date: NaiveDate, impressions: u64, clicks: u64, spend: f64, conversions: u64, revenue: f64, actor: &str) -> Result<MetricRecord, String> {
        if self.get_ad_group(ad_group_id).is_none() { return Err(format!("Ad group not found: {ad_group_id}")); }
        let rec = MetricRecord { ad_group_id: ad_group_id.to_string(), date, impressions, clicks, spend, conversions, revenue };
        let mut metrics = self.metrics.lock().unwrap();
        metrics.retain(|m| !(m.ad_group_id == ad_group_id && m.date == date));
        metrics.push(rec.clone());
        drop(metrics);
        self.audit(actor, "ingest_metrics", format!("{ad_group_id} {date}"));
        Ok(rec)
    }

    fn aggregate(records: &[&MetricRecord]) -> (u64, u64, f64, u64, f64) {
        let imp = records.iter().map(|m| m.impressions).sum();
        let clk = records.iter().map(|m| m.clicks).sum();
        let spend = records.iter().map(|m| m.spend).sum();
        let conv = records.iter().map(|m| m.conversions).sum();
        let rev = records.iter().map(|m| m.revenue).sum();
        (imp, clk, spend, conv, rev)
    }

    /// Derived KPIs from a set of metric records.
    fn kpis(imp: u64, clk: u64, spend: f64, conv: u64, rev: f64) -> serde_json::Value {
        let ctr = if imp > 0 { clk as f64 / imp as f64 * 100.0 } else { 0.0 };
        let cpc = if clk > 0 { spend / clk as f64 } else { 0.0 };
        let cpa = if conv > 0 { spend / conv as f64 } else { 0.0 };
        let cvr = if clk > 0 { conv as f64 / clk as f64 * 100.0 } else { 0.0 };
        let roas = if spend > 0.0 { rev / spend } else { 0.0 };
        serde_json::json!({
            "impressions": imp,
            "clicks": clk,
            "spend": round2(spend),
            "conversions": conv,
            "revenue": round2(rev),
            "ctr_pct": round2(ctr),
            "cpc": round2(cpc),
            "cpa": round2(cpa),
            "cvr_pct": round2(cvr),
            "roas": round2(roas),
        })
    }

    /// Performance for one ad group (all recorded days).
    pub fn ad_group_performance(&self, ad_group_id: &str) -> Option<serde_json::Value> {
        let g = self.get_ad_group(ad_group_id)?;
        let metrics = self.metrics.lock().unwrap();
        let recs: Vec<&MetricRecord> = metrics.iter().filter(|m| m.ad_group_id == ad_group_id).collect();
        let (imp, clk, spend, conv, rev) = Self::aggregate(&recs);
        let mut out = Self::kpis(imp, clk, spend, conv, rev);
        if let Some(o) = out.as_object_mut() {
            o.insert("ad_group_id".into(), serde_json::json!(g.id));
            o.insert("name".into(), serde_json::json!(g.name));
            o.insert("days".into(), serde_json::json!(recs.len()));
        }
        Some(out)
    }

    /// Roll up performance across all ad groups in a campaign.
    pub fn campaign_performance(&self, campaign_id: &str) -> Option<serde_json::Value> {
        let c = self.get_campaign(campaign_id)?;
        let group_ids: Vec<String> = self.ad_groups_for(campaign_id).into_iter().map(|g| g.id).collect();
        let metrics = self.metrics.lock().unwrap();
        let recs: Vec<&MetricRecord> = metrics.iter().filter(|m| group_ids.contains(&m.ad_group_id)).collect();
        let (imp, clk, spend, conv, rev) = Self::aggregate(&recs);
        let mut out = Self::kpis(imp, clk, spend, conv, rev);
        if let Some(o) = out.as_object_mut() {
            o.insert("campaign_id".into(), serde_json::json!(c.id));
            o.insert("name".into(), serde_json::json!(c.name));
            o.insert("ad_groups".into(), serde_json::json!(group_ids.len()));
        }
        Some(out)
    }

    /// Budget pacing: today's spend vs. daily budget for a campaign.
    pub fn budget_pacing(&self, campaign_id: &str) -> Option<serde_json::Value> {
        let c = self.get_campaign(campaign_id)?;
        let group_ids: Vec<String> = self.ad_groups_for(campaign_id).into_iter().map(|g| g.id).collect();
        let today = Utc::now().date_naive();
        let metrics = self.metrics.lock().unwrap();
        let today_spend: f64 = metrics.iter().filter(|m| group_ids.contains(&m.ad_group_id) && m.date == today).map(|m| m.spend).sum();
        let pct = if c.daily_budget > 0.0 { today_spend / c.daily_budget * 100.0 } else { 0.0 };
        let status = if pct >= 100.0 { "budget_exhausted" } else if pct >= 80.0 { "pacing_hot" } else if pct < 30.0 { "underspending" } else { "on_track" };
        Some(serde_json::json!({
            "campaign_id": c.id,
            "daily_budget": c.daily_budget,
            "spend_today": round2(today_spend),
            "pace_pct": round2(pct),
            "pacing": status,
        }))
    }

    /// Rule-based optimization suggestions for a campaign's ad groups, using
    /// lifetime KPIs vs. simple thresholds + bid-strategy targets. Powers the
    /// Campaign Optimizer agent (it suggests; spend changes stay gated).
    pub fn optimize(&self, campaign_id: &str) -> Option<serde_json::Value> {
        let _c = self.get_campaign(campaign_id)?;
        let groups = self.ad_groups_for(campaign_id);
        let metrics = self.metrics.lock().unwrap();
        let mut suggestions = Vec::new();
        for g in &groups {
            let recs: Vec<&MetricRecord> = metrics.iter().filter(|m| m.ad_group_id == g.id).collect();
            if recs.is_empty() { continue; }
            let (imp, clk, spend, conv, rev) = Self::aggregate(&recs);
            let ctr = if imp > 0 { clk as f64 / imp as f64 * 100.0 } else { 0.0 };
            let cpa = if conv > 0 { spend / conv as f64 } else { f64::INFINITY };
            let roas = if spend > 0.0 { rev / spend } else { 0.0 };
            let mut recs_out: Vec<String> = Vec::new();
            // Conversion efficiency.
            if conv == 0 && spend > 0.0 { recs_out.push("no conversions despite spend — pause or revise targeting/creative".into()); }
            if roas >= 3.0 { recs_out.push(format!("strong ROAS {:.2} — consider raising bid/budget to scale", roas)); }
            else if roas > 0.0 && roas < 1.0 { recs_out.push(format!("ROAS {:.2} below break-even — lower bid or pause", roas)); }
            // CTR (creative relevance).
            if imp >= 1000 && ctr < 1.0 { recs_out.push(format!("low CTR {:.2}% — refresh creative or tighten audience", ctr)); }
            // Target-CPA strategy adherence.
            if g.bid_strategy == BidStrategy::TargetCpa && cpa.is_finite() && cpa > g.bid * 1.25 {
                recs_out.push(format!("CPA {:.2} exceeds target {:.2} by >25% — lower target or improve landing page", cpa, g.bid));
            }
            if recs_out.is_empty() { recs_out.push("performing within thresholds — hold".into()); }
            suggestions.push(serde_json::json!({
                "ad_group_id": g.id,
                "name": g.name,
                "bid_strategy": g.bid_strategy,
                "ctr_pct": round2(ctr),
                "cpa": if cpa.is_finite() { serde_json::json!(round2(cpa)) } else { serde_json::Value::Null },
                "roas": round2(roas),
                "suggestions": recs_out,
            }));
        }
        Some(serde_json::json!({"campaign_id": campaign_id, "ad_groups_analyzed": suggestions.len(), "recommendations": suggestions}))
    }

    pub fn audit_log(&self, limit: usize) -> Vec<AuditEntry> {
        let log = self.audit_log.lock().unwrap();
        log.iter().rev().take(limit).cloned().collect()
    }

    // ─── seed ────────────────────────────────────────────────────────────

    fn seed(&self) {
        let today = Utc::now().date_naive();

        // Audience.
        let aud = self.create_audience("In-market shoppers", "interest", "interest:electronics", 2_400_000, "system");

        // A sales campaign with two ad groups (one strong, one weak).
        let camp = self.create_campaign("Q2 Electronics Sale", "search", Objective::Sales, 500.0, "USD", today - Duration::days(14), None, "system");
        self.set_campaign_status(&camp.id, CampaignStatus::Active, "system").ok();

        let strong = self.create_ad_group(&camp.id, "Branded - High Intent", BidStrategy::TargetRoas, 4.0, Some(aud.id.clone()), "system").unwrap();
        let weak = self.create_ad_group(&camp.id, "Broad - Prospecting", BidStrategy::TargetCpa, 25.0, None, "system").unwrap();

        self.create_ad(&strong.id, "Save 20% on Laptops", "Top brands, free shipping", "https://example.com/laptops", "system").ok();
        self.create_ad(&weak.id, "Shop Electronics", "Great deals daily", "https://example.com/shop", "system").ok();

        // Metrics: strong group has good ROAS; weak group overspends with no
        // conversions. 7 days of clean daily figures.
        let mut metrics = self.metrics.lock().unwrap();
        for d in 0..7 {
            let date = today - Duration::days(6 - d);
            // strong: 10k imp, 400 clk (4% CTR), $200 spend, 40 conv, $1000 rev -> ROAS 5
            metrics.push(MetricRecord { ad_group_id: strong.id.clone(), date, impressions: 10_000, clicks: 400, spend: 200.0, conversions: 40, revenue: 1000.0 });
            // weak: 8k imp, 40 clk (0.5% CTR), $300 spend, 0 conv, $0 rev
            metrics.push(MetricRecord { ad_group_id: weak.id.clone(), date, impressions: 8_000, clicks: 40, spend: 300.0, conversions: 0, revenue: 0.0 });
        }
        drop(metrics);

        // A second, paused awareness campaign (display).
        let aware = self.create_campaign("Brand Awareness - Display", "display", Objective::Awareness, 150.0, "USD", today - Duration::days(30), None, "system");
        self.set_campaign_status(&aware.id, CampaignStatus::Paused, "system").ok();
        self.create_ad_group(&aware.id, "Display - Lookalike", BidStrategy::MaxConversions, 2.0, None, "system").ok();
    }
}

// ─── helper ──────────────────────────────────────────────────────────────

fn round2(x: f64) -> f64 { (x * 100.0).round() / 100.0 }
