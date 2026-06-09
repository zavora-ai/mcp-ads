//! Integration tests: KPI math, budget/bid guards, pacing, and optimization
//! suggestions (scale winners, cut/pause losers, CTR/CPA rules).

use chrono::Utc;
use mcp_ads::store::AdsStore;
use mcp_ads::types::*;

fn store() -> AdsStore {
    AdsStore::new()
}

fn camp(s: &AdsStore) -> String {
    s.list_campaigns(None, None).into_iter().find(|c| c.name.contains("Electronics")).unwrap().id
}

#[test]
fn seed_loads() {
    let s = store();
    assert!(s.list_campaigns(None, None).len() >= 2);
    assert!(!s.list_audiences().is_empty());
}

#[test]
fn kpis_computed_correctly() {
    let s = store();
    let c = camp(&s);
    let groups = s.ad_groups_for(&c);
    let strong = groups.iter().find(|g| g.name.contains("Branded")).unwrap();
    let perf = s.ad_group_performance(&strong.id).unwrap();
    // 7 days x (10k imp, 400 clk, $200, 40 conv, $1000) -> CTR 4%, ROAS 5, CPA 5
    assert_eq!(perf["ctr_pct"].as_f64().unwrap(), 4.0);
    assert_eq!(perf["roas"].as_f64().unwrap(), 5.0);
    assert_eq!(perf["cpa"].as_f64().unwrap(), 5.0);
    assert_eq!(perf["conversions"].as_u64().unwrap(), 280);
}

#[test]
fn campaign_rollup() {
    let s = store();
    let c = camp(&s);
    let perf = s.campaign_performance(&c).unwrap();
    // strong $200/day + weak $300/day over 7 days = $3500
    assert_eq!(perf["spend"].as_f64().unwrap(), 3500.0);
    assert_eq!(perf["ad_groups"].as_u64().unwrap(), 2);
}

#[test]
fn budget_and_bid_guards() {
    let s = store();
    let c = camp(&s);
    assert!(s.set_budget(&c, -10.0, "opt").is_err());
    let ok = s.set_budget(&c, 750.0, "opt").unwrap();
    assert_eq!(ok.daily_budget, 750.0);
    let g = s.ad_groups_for(&c)[0].clone();
    assert!(s.set_bid(&g.id, -1.0, "opt").is_err());
    assert_eq!(s.set_bid(&g.id, 6.0, "opt").unwrap().bid, 6.0);
}

#[test]
fn status_workflow_and_ended_guard() {
    let s = store();
    let c = camp(&s);
    s.set_campaign_status(&c, CampaignStatus::Paused, "opt").unwrap();
    assert_eq!(s.get_campaign(&c).unwrap().status, CampaignStatus::Paused);
    s.set_campaign_status(&c, CampaignStatus::Ended, "opt").unwrap();
    // ended is terminal
    assert!(s.set_campaign_status(&c, CampaignStatus::Active, "opt").is_err());
}

#[test]
fn pacing_flags() {
    let s = store();
    let c = camp(&s);
    // add today's spend to trip pacing: strong+weak today already seeded ($500 today vs $500 budget)
    let pacing = s.budget_pacing(&c).unwrap();
    // $200 + $300 = $500 today against $500 budget -> ~100% -> exhausted
    assert!(pacing["pace_pct"].as_f64().unwrap() >= 99.0);
    assert_eq!(pacing["pacing"], "budget_exhausted");
}

#[test]
fn optimize_scales_winner_and_cuts_loser() {
    let s = store();
    let c = camp(&s);
    let opt = s.optimize(&c).unwrap();
    let recs = opt["recommendations"].as_array().unwrap();
    let strong = recs.iter().find(|r| r["name"].as_str().unwrap().contains("Branded")).unwrap();
    let weak = recs.iter().find(|r| r["name"].as_str().unwrap().contains("Broad")).unwrap();
    // strong ROAS 5 -> scale suggestion
    let strong_text = strong["suggestions"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect::<Vec<_>>().join(" ");
    assert!(strong_text.contains("scale") || strong_text.contains("raising"), "got: {strong_text}");
    // weak: 0 conversions + low CTR -> cut/pause
    let weak_text = weak["suggestions"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect::<Vec<_>>().join(" ");
    assert!(weak_text.contains("no conversions") || weak_text.contains("pause"), "got: {weak_text}");
    assert!(weak_text.contains("CTR"), "low CTR should be flagged: {weak_text}");
}

#[test]
fn create_hierarchy_and_metrics() {
    let s = store();
    let c = s.create_campaign("New", "social", Objective::Traffic, 100.0, "USD", Utc::now().date_naive(), None, "u");
    let g = s.create_ad_group(&c.id, "G1", BidStrategy::ManualCpc, 1.5, None, "u").unwrap();
    s.create_ad(&g.id, "Hi", "body", "https://x", "u").unwrap();
    s.ingest_metrics(&g.id, Utc::now().date_naive(), 1000, 50, 75.0, 5, 250.0, "u").unwrap();
    let perf = s.ad_group_performance(&g.id).unwrap();
    assert_eq!(perf["clicks"].as_u64().unwrap(), 50);
    assert_eq!(perf["roas"].as_f64().unwrap(), 3.33); // 250/75, rounded to 2dp
}

#[test]
fn ad_group_requires_campaign() {
    let s = store();
    assert!(s.create_ad_group("CMP-nope", "x", BidStrategy::ManualCpc, 1.0, None, "u").is_err());
}
