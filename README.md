# Ads MCP Server

[![Crates.io](https://img.shields.io/crates/v/mcp-ads.svg)](https://crates.io/crates/mcp-ads)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![ADK-Rust Enterprise](https://img.shields.io/badge/ADK--Rust-Enterprise-purple.svg)](https://enterprise.adk-rust.com)
[![Registry Ready](https://img.shields.io/badge/ADK_Registry-Ready-green.svg)](https://www.zavora.ai)

A digital advertising platform for [ADK-Rust Enterprise](https://enterprise.adk-rust.com) sales & marketing agents. 19 MCP tools covering campaigns, ad groups, ads/creatives, **budgets and bids**, audiences, daily performance metrics with **derived KPIs (CTR/CPC/CPA/CVR/ROAS)**, budget pacing, and **optimization suggestions** — over an audit trail, with approval gates on every spend-affecting change.

## A platform, not a point solution

This is modeled as a general ads-management backbone (à la Google Ads / Meta Ads Manager), so marketing agents are clients of one shared system:

| Agent | Domain | Uses |
|-------|--------|------|
| **Campaign Optimizer** | sales-marketing | `campaign_performance`, `budget_pacing`, `optimize`, then gated `set_bid` / `set_budget` / `set_campaign_status` |

The optimizer **reads** performance and **suggests** actions; the actions that move money (bid, budget, status) are separate, approval-gated tools — so analysis is free-flowing while spend stays governed.

## Architecture

<p align="center">
  <img src="https://raw.githubusercontent.com/zavora-ai/mcp-ads/main/docs/architecture.svg" alt="Ads MCP Architecture" width="780"/>
</p>

## Capabilities

- **Campaign hierarchy** — campaigns (objective, channel, daily budget) → ad groups (bid strategy + bid/target, audience) → ads/creatives.
- **Audiences** — interest / remarketing / lookalike / custom with estimated size.
- **Metrics & KPIs** — ingest daily impressions/clicks/spend/conversions/revenue; derived **CTR, CPC, CPA, CVR, ROAS** at ad-group and rolled-up campaign level.
- **Budget pacing** — today's spend vs. daily budget with a pacing flag (underspending / on_track / pacing_hot / budget_exhausted).
- **Optimization** — rule-based suggestions per ad group: scale high-ROAS winners, cut/pause sub-break-even or zero-conversion groups, flag low CTR, and catch CPA running over a target-CPA strategy's target.

## Governance posture

- **Three writes directly affect spend and are gated** (`requires_approval`, `external_write`): `set_budget`, `set_bid`, and `set_campaign_status` (pausing/activating a campaign changes whether it spends). Creating entities and ingesting metrics are normal internal writes.
- **`optimize` is read-only** — it recommends; applying a recommendation goes through a gated tool. Budgets and bids are guarded non-negative; ended campaigns are terminal.
- Everything material is on the audit trail (`audit_log`). Sample data is fictitious.

## Tools (19)

### Campaigns (5)
`create_campaign` · `get_campaign` · `list_campaigns` · `set_campaign_status` (gated) · `set_budget` (gated)

### Ad Groups & Ads (6)
`create_ad_group` · `get_ad_group` · `list_ad_groups` · `set_bid` (gated) · `create_ad` · `list_ads`

### Audiences & Metrics (3)
`create_audience` · `list_audiences` · `ingest_metrics`

### Performance & Optimization (5)
`ad_group_performance` · `campaign_performance` · `budget_pacing` · `optimize` · `audit_log`

## Example

```jsonc
// Campaign Optimizer: read performance, get suggestions, then act (gated)
{"name": "campaign_performance", "arguments": {"campaign_id": "CMP-1002"}}
{"name": "optimize", "arguments": {"campaign_id": "CMP-1002"}}
{"name": "set_bid", "arguments": {"ad_group_id": "ADG-1003", "bid": 5.0}}
{"name": "set_budget", "arguments": {"campaign_id": "CMP-1002", "daily_budget": 800}}
```

## Install & run

```bash
cargo install mcp-ads
mcp-ads            # serves MCP over stdio
```

Or build from source:

```bash
git clone https://github.com/zavora-ai/mcp-ads
cd mcp-ads && cargo build --release
./target/release/mcp-ads
```

## Registry manifest

```toml
server_id = "mcp_ads"
display_name = "Ads / Advertising"
version = "1.0.0"
domain = "sales-marketing"
risk_level = "high"
writes_allowed = "gated"
```

The full [`mcp-server.toml`](mcp-server.toml) declares all 19 tools with risk classes and approval gates for registry onboarding.

## License

Apache-2.0
