# Experiment Reports

Chronological record of Telos validation experiments. Each experiment uses the `telos-experiment` framework to evaluate whether structured Telos context improves LLM agent code review compared to Git-only and flat CONSTRAINTS.md baselines.

## Reports

| Date | Report | Agent | N | Key Finding |
|------|--------|-------|---|-------------|
| 2026-03-01 | [Baseline Agent Experiment](2026-03-01-baseline-agent-experiment.md) | codex-cli 0.96.0 | 2 | TP detection saturated across conditions; FP rate high (50-67%); Telos improves constraint citation |

## Raw Data

Each report links to a companion JSON file with full trial-level data:

- `2026-03-01-baseline-results.json` — 42 trials, 7 scenarios × 3 conditions × 2 repeats
