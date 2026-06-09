# Changelog

## [1.0.0] - 2026-06-10

Initial release — a broad digital advertising platform.

### Added
- **Campaign hierarchy** — campaigns → ad groups (bid strategy + bid) → ads/creatives
  (`create_campaign`, `get_campaign`, `list_campaigns`, `create_ad_group`, `get_ad_group`, `list_ad_groups`, `create_ad`, `list_ads`)
- **Audiences** — interest/remarketing/lookalike/custom
  (`create_audience`, `list_audiences`)
- **Metrics & KPIs** — daily metric ingest; derived CTR/CPC/CPA/CVR/ROAS at ad-group and campaign level
  (`ingest_metrics`, `ad_group_performance`, `campaign_performance`)
- **Pacing & optimization** — today's spend vs budget with a pacing flag; rule-based optimization suggestions (scale winners, cut losers, low-CTR/CPA-over-target)
  (`budget_pacing`, `optimize`, `audit_log`)
- **Spend governance** — `set_budget`, `set_bid`, and `set_campaign_status` are `external_write` + approval-gated; budgets/bids guarded non-negative; ended campaigns terminal.
- 19 tools total; full audit trail.
- 13 tests (9 integration + 4 manifest); verified end-to-end over MCP stdio.
