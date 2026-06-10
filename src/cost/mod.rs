// ─── Cost comparison: Sparrow vs competitors (§ cost-routing) ──────────────────
//
// Every run tracks its own token usage and cost. This module answers:
//   "What would this exact run have cost on Claude Code / Codex / OpenCode?"
//
// The answer is Sparrow's competitive moat — because Claude Code and Codex sell
// their own tokens (structural conflict of interest), they can never route a
// trivial "read this file" to a free local model. Sparrow does.

use crate::event::TokenUsage;

// ─── Competitor catalogue ───────────────────────────────────────────────────────

/// A competing AI coding tool with known per-token pricing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Competitor {
    ClaudeCode, // Anthropic Claude Code (uses Claude Sonnet pricing)
    Codex,      // OpenAI Codex CLI (uses GPT-4o pricing)
    OpenCode,   // OpenCode (typically Claude/GPT routing)
    Cursor,     // Cursor (GPT-4o / Claude)
    Copilot,    // GitHub Copilot (GPT-4o / Claude)
    Aider,      // Aider (user's own API key — variable, use market avg)
}

impl Competitor {
    /// Human-readable name for display.
    pub fn display_name(&self) -> &str {
        match self {
            Competitor::ClaudeCode => "Claude Code",
            Competitor::Codex => "Codex CLI",
            Competitor::OpenCode => "OpenCode",
            Competitor::Cursor => "Cursor",
            Competitor::Copilot => "GitHub Copilot",
            Competitor::Aider => "Aider",
        }
    }

    /// Cost per million tokens — (input, output) in USD.
    /// Prices as of 2026-06. Conservative estimates (list price, no bulk discount).
    pub fn price_per_mtok(&self) -> (f64, f64) {
        match self {
            // Claude 3.5/4 Sonnet pricing (what Claude Code uses)
            Competitor::ClaudeCode => (3.0, 15.0),
            // GPT-4o pricing (what Codex uses)
            Competitor::Codex => (2.5, 10.0),
            // OpenCode typically routes to Claude or GPT-4o — use Claude pricing
            Competitor::OpenCode => (3.0, 15.0),
            // Cursor Pro — GPT-4o / Claude 3.5 Sonnet
            Competitor::Cursor => (2.5, 10.0),
            // GitHub Copilot — uses GPT-4o / Claude 3.5
            Competitor::Copilot => (2.5, 10.0),
            // Aider — BYO key, use market-heavyweight average (Claude)
            Competitor::Aider => (3.0, 15.0),
        }
    }

    /// Compute what this competitor would have charged for the given token usage.
    pub fn estimate_cost(&self, tokens: &TokenUsage) -> f64 {
        let (in_price, out_price) = self.price_per_mtok();
        let input_cost = tokens.input as f64 * in_price / 1_000_000.0;
        let output_cost = tokens.output as f64 * out_price / 1_000_000.0;
        input_cost + output_cost
    }
}

// ─── Comparison engine ───────────────────────────────────────────────────────────

/// A single competitor-vs-Sparrow cost line.
#[derive(Debug, Clone)]
pub struct CostLine {
    pub competitor: Competitor,
    pub estimated_cost: f64,
    pub savings: f64, // Sparrow cost minus competitor cost (negative = Sparrow cheaper)
    pub savings_percent: f64, // e.g. 93.0 = Sparrow is 93% cheaper
}

/// Builds a full cost comparison report.
///
/// `sparrow_cost` is the ACTUAL cost from the Sparrow run.
/// `tokens` is the token usage from the run (used to estimate competitor cost).
pub fn compare(sparrow_cost: f64, tokens: &TokenUsage) -> Vec<CostLine> {
    let competitors = [
        Competitor::ClaudeCode,
        Competitor::Codex,
        Competitor::OpenCode,
    ];

    competitors
        .iter()
        .map(|c| {
            let estimated = c.estimate_cost(tokens);
            let savings = sparrow_cost - estimated; // negative = Sparrow cheaper
            let savings_percent = if estimated > 0.0 {
                ((estimated - sparrow_cost) / estimated * 100.0).max(0.0)
            } else {
                0.0
            };
            CostLine {
                competitor: *c,
                estimated_cost: estimated,
                savings,
                savings_percent,
            }
        })
        .collect()
}

/// Format a dollar amount without lying at small scales: sub-cent amounts
/// keep 4 decimals instead of rounding to a silly "$0.00".
fn fmt_usd(amount: f64) -> String {
    if amount.abs() >= 0.01 {
        format!("${:.2}", amount)
    } else {
        format!("${:.4}", amount)
    }
}

/// Format a cost comparison report as a string suitable for terminal output.
///
/// Example output:
///   ── Cost ──────────────────────────────────────────
///   Sparrow .............. $0.0412 (2,412 in / 847 out)
///   Claude Code .......... $0.6130 (save 93% — $0.57 cheaper)
///   Codex CLI ............ $0.4150 (save 90% — $0.37 cheaper)
///   OpenCode ............. $0.6130 (save 93% — $0.57 cheaper)
pub fn format_comparison(sparrow_cost: f64, tokens: &TokenUsage) -> String {
    let lines = compare(sparrow_cost, tokens);
    let mut out = String::new();

    out.push_str("── Cost ──────────────────────────────────────────\n");
    out.push_str(&format!(
        "Sparrow .............. ${:.4} ({} in / {} out)\n",
        sparrow_cost, tokens.input, tokens.output
    ));

    for line in &lines {
        if line.savings_percent > 0.0 {
            out.push_str(&format!(
                "{} .......... ~${:.4} est. (save {:.0}% — {} cheaper)\n",
                line.competitor.display_name(),
                line.estimated_cost,
                line.savings_percent,
                fmt_usd(-line.savings)
            ));
        } else {
            // Sparrow was not cheaper on this run (e.g. the task was routed to
            // the same premium model). Say so honestly instead of "same cost".
            out.push_str(&format!(
                "{} .......... ~${:.4} est. (comparable on this run)\n",
                line.competitor.display_name(),
                line.estimated_cost
            ));
        }
    }

    // Killer line — the marketing hook. Estimates only: same token volume
    // priced at the competitor's list price, so keep the tilde.
    if let Some(best) = lines
        .iter()
        .max_by(|a, b| a.savings_percent.partial_cmp(&b.savings_percent).unwrap())
    {
        if best.savings_percent > 50.0 {
            out.push_str(&format!(
                "\n💡 Same tokens on {} would have cost ~{} more (est. at list price).",
                best.competitor.display_name(),
                fmt_usd(-best.savings)
            ));
        }
    }

    out
}

/// One-liner for compact display (chat, subagent output).
pub fn format_comparison_oneliner(sparrow_cost: f64, tokens: &TokenUsage) -> String {
    let lines = compare(sparrow_cost, tokens);
    let best = lines
        .iter()
        .max_by(|a, b| a.savings_percent.partial_cmp(&b.savings_percent).unwrap());

    match best {
        Some(line) if line.savings_percent > 30.0 => format!(
            "(vs {}: ~{} est. — save {:.0}%)",
            line.competitor.display_name(),
            fmt_usd(line.estimated_cost),
            line.savings_percent
        ),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_competitor_pricing_known() {
        let cc = Competitor::ClaudeCode.price_per_mtok();
        assert_eq!(cc, (3.0, 15.0));

        let cx = Competitor::Codex.price_per_mtok();
        assert_eq!(cx, (2.5, 10.0));
    }

    #[test]
    fn test_estimate_cost_zero_tokens() {
        let tokens = TokenUsage {
            input: 0,
            output: 0,
        };
        assert_eq!(Competitor::ClaudeCode.estimate_cost(&tokens), 0.0);
    }

    #[test]
    fn test_estimate_cost_typical_run() {
        // Typical small coding task: 5k input, 1k output
        let tokens = TokenUsage {
            input: 5_000,
            output: 1_000,
        };
        let cost = Competitor::ClaudeCode.estimate_cost(&tokens);
        // input: 5000 * 3.0 / 1_000_000 = 0.015
        // output: 1000 * 15.0 / 1_000_000 = 0.015
        // total = 0.030
        assert!((cost - 0.030).abs() < 0.001);
    }

    #[test]
    fn test_compare_returns_three_competitors() {
        let tokens = TokenUsage {
            input: 10_000,
            output: 2_000,
        };
        let lines = compare(0.05, &tokens);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_format_comparison_shows_savings() {
        let tokens = TokenUsage {
            input: 10_000,
            output: 2_000,
        };
        let report = format_comparison(0.02, &tokens);
        assert!(report.contains("Sparrow"));
        assert!(report.contains("Claude Code"));
        assert!(report.contains("save"));
        assert!(report.contains("cheaper"));
    }

    #[test]
    fn test_oneliner_includes_savings() {
        let tokens = TokenUsage {
            input: 100_000,
            output: 20_000,
        };
        let oneliner = format_comparison_oneliner(0.10, &tokens);
        assert!(oneliner.contains("save"));
        assert!(oneliner.contains("%"));
    }
}
