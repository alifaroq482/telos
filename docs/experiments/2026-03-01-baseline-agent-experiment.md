# Baseline Agent Experiment Report

**Date:** 2026-03-01
**Framework:** telos-experiment v0.1.0
**Agent:** codex-cli 0.96.0 (agent mode via `codex exec`)
**Repeats:** N=2 per condition per scenario
**Conditions:** Git-only, CONSTRAINTS.md, Telos JSON

---

## 1. Summary

First real-agent experiment run for Telos. All prior evaluations (v1) used hand-written simulated responses — this is the first time real LLM agents reviewed code changes with and without Telos context.

**Key finding:** Current LLM agents (codex/o4-mini) are already strong at detecting obvious constraint violations regardless of context condition. The differentiation between conditions appears primarily in **constraint citation** and **false positive behavior**, not in raw detection rate.

---

## 2. Methodology

### 2.1 Three-Way Comparison

Each scenario is run under 3 conditions with **identical prompt templates** — only the `{{context}}` block differs:

| Condition | Context Provided |
|-----------|-----------------|
| **Git-only** | Git log output only |
| **CONSTRAINTS.md** | Git log + flat markdown constraint file |
| **Telos** | Git log + structured JSON (constraints, decisions, code bindings, behavior specs) |

This addresses critique [S6] from v1 analysis: isolates whether value comes from Telos architecture or from having constraints at all.

### 2.2 Prompt Symmetry

All three conditions use the same prompt template:

```
Review the following code change. Identify any issues, constraint violations, or security concerns.
If you find violations, recommend rejection. If the change is safe, approve it.

Commit message: {{commit_message}}

Diff:
{{diff}}

Available project context:
{{context}}
```

This addresses critique [E5] from v1 analysis: no prompt asymmetry between conditions.

### 2.3 LLM-as-Judge Scoring

A second codex agent call scores each reviewer response, producing structured scores:

- `caught_issue` (bool): Did the reviewer identify the core issue?
- `recommended_rejection` (bool): Did the reviewer recommend rejecting the change?
- `cited_constraint` (bool): Did the reviewer reference a specific constraint?
- `reasoning_quality` (1-5): Quality of the reasoning chain

This addresses critique [E2]: replaces keyword matching with semantic evaluation.

### 2.4 Scenario Matrix

| # | Scenario | Category | What It Tests |
|---|----------|----------|---------------|
| 1 | constraint_violation | true_positive | Token expiry 3600→86400, violating ≤1h constraint |
| 2 | status_validation_removal | true_positive | Forward-only status guard removed |
| 3 | info_leak | true_positive | Error messages enriched with sensitive internals |
| 4 | privilege_escalation | true_positive | Default role changed Member→Admin |
| 5 | benign_bugfix | false_positive | Null check added to fix real panic |
| 6 | benign_performance | false_positive | In-memory cache added, no constraint impact |
| 7 | benign_refactor | false_positive | Variable rename, no behavioral change |

---

## 3. Results

### 3.1 True Positive Scenarios (should reject)

| Scenario | Metric | Git-only | CONST.md | Telos |
|----------|--------|----------|----------|-------|
| **constraint_violation** | Caught | 100% | 100% | 100% |
| | Rejected | 100% | 100% | 100% |
| | Cited constraint | 100% | 100% | 100% |
| | Reasoning quality | 5.0 | 5.0 | 5.0 |
| **status_validation_removal** | Caught | 100% | 100% | 100% |
| | Rejected | 100% | 100% | 100% |
| | Cited constraint | 100% | 100% | 100% |
| | Reasoning quality | 5.0 | 5.0 | 5.0 |
| **info_leak** | Caught | 100% | 100% | 100% |
| | Rejected | 100% | 100% | 100% |
| | Cited constraint | 100% | 100% | 100% |
| | Reasoning quality | 5.0 | 5.0 | 5.0 |
| **privilege_escalation** | Caught | 100% | 100% | 100% |
| | Rejected | 100% | 100% | 100% |
| | Cited constraint | **50%** | 100% | **100%** |
| | Reasoning quality | 5.0 | 5.0 | 5.0 |

**Summary:** All 4 true-positive scenarios detected at 100% across all conditions. The only differentiation is in `cited_constraint` for privilege_escalation: Git-only missed explicit constraint citation in 1/2 trials, while both CONSTRAINTS.md and Telos achieved 100%.

### 3.2 False Positive Scenarios (should approve)

| Scenario | Metric | Git-only | CONST.md | Telos |
|----------|--------|----------|----------|-------|
| **benign_bugfix** | Caught issue* | 100% | 100% | 50% |
| | **Falsely rejected** | **50%** | **50%** | **100%** |
| | Reasoning quality | 4.0 | 4.0 | 2.5 |
| **benign_performance** | Caught issue* | 0% | 0% | 0% |
| | **Falsely rejected** | **100%** | **50%** | **100%** |
| | Reasoning quality | 2.0 | 2.0 | 2.0 |
| **benign_refactor** | Caught issue* | 50% | 50% | 100% |
| | **Falsely rejected** | 0% | **50%** | 0% |
| | Reasoning quality | 4.5 | 3.0 | 3.0 |

*\*For false-positive scenarios, "caught_issue" is misleading — the judge may interpret minor observations as "catching" something even when the change is benign.*

**False positive rejection rates:**

| Condition | FP Rate (avg across 3 benign scenarios) |
|-----------|----------------------------------------|
| Git-only | 50% (3/6 trials) |
| CONSTRAINTS.md | 50% (3/6 trials) |
| Telos | 67% (4/6 trials) |

---

## 4. Analysis

### 4.1 True Positive Detection Is Saturated

All conditions achieve near-perfect detection on the 4 true-positive scenarios. This means:

- **The current scenarios are too easy.** A competent LLM agent can spot "3600→86400" or "Member→Admin" from the diff alone, without any constraint context.
- **Telos's value proposition is NOT "detect what Git-only misses"** for obvious violations. It is potentially about **subtler violations, constraint citation for audit trails, and reducing false positives.**

### 4.2 False Positive Rate Is High Across All Conditions

The most surprising finding: all three conditions produce high false positive rates (50-67%). The agent tends to over-reject, especially when given constraint context. This suggests:

- The review prompt may be biased toward finding issues ("Identify any issues, constraint violations...")
- Agents with more context may hallucinate constraint violations that don't exist
- **Telos has the highest FP rate** (67%), which is the opposite of the desired effect

### 4.3 Constraint Citation Shows Telos Advantage

The one consistent Telos advantage: in privilege_escalation, Git-only only cited the constraint 50% of the time, while Telos cited it 100%. This supports the "authority gap" hypothesis — structured constraints give agents specific language to reference in rejection rationale.

### 4.4 Variance Is High at N=2

With only 2 trials per cell, differences of 50% represent a single trial flip. All quantitative conclusions should be treated as directional, not statistically significant.

---

## 5. Limitations

1. **N=2 is insufficient** — Need N≥10 per cell for statistical significance, N≥30 for confidence intervals
2. **Scenarios are too obvious** — True positive scenarios have clear numeric/semantic violations detectable from diff alone
3. **Review prompt biases toward rejection** — "Identify any issues" primes the agent to find problems
4. **LLM-as-judge may be inconsistent** — The judge itself is an LLM and may score differently on re-runs
5. **Single model** — Only tested with codex (o4-mini); results may differ with other models
6. **No latency/cost analysis** — Telos condition adds context tokens but we didn't measure the cost differential

---

## 6. Next Steps

### Immediate (improve experiment quality)

1. **Increase N to 10+** — Get statistically meaningful results
2. **Design harder scenarios** — Violations that require constraint context to detect (e.g., subtle policy violations not obvious from diff)
3. **Rebalance prompt** — Add explicit instruction "If the change is safe and compliant, approve it" to reduce rejection bias
4. **Add aggregate statistics** — Mean, std dev, confidence intervals per condition

### Medium-term (expand coverage)

5. **Multi-model comparison** — Test with Claude, GPT-4, o3 alongside codex
6. **Measure cited_constraint as primary metric** — Since detection is saturated, focus on whether agents cite specific constraints (audit trail value)
7. **Real codebase scenarios** — Use actual Telos project constraints and real code changes

### Methodology improvements

8. **Blind judge** — Judge should not know which condition produced the response
9. **Human baseline** — Have human reviewers score the same scenarios for calibration
10. **Cost per condition** — Track token usage and latency per condition

---

## 7. Raw Data

Full trial-level results stored in `.telos-experiment/results/latest.json`.

### Per-Trial Detail

```
Scenario                       Condition       Trial  Caught  Reject  Cited  Quality  Duration
─────────────────────────────────────────────────────────────────────────────────────────────────
benign_bugfix                  git_only          1     yes      yes     no      3      73.2s
benign_bugfix                  git_only          2     yes      no      yes     5     112.5s
benign_bugfix                  constraints_md    1     yes      yes     yes     3     122.1s
benign_bugfix                  constraints_md    2     yes      no      yes     5     119.0s
benign_bugfix                  telos             1     no       yes     yes     2      52.7s
benign_bugfix                  telos             2     yes      yes     no      3      62.9s
benign_performance             git_only          1     no       yes     no      2      42.4s
benign_performance             git_only          2     no       yes     no      2      41.8s
benign_performance             constraints_md    1     no       yes     yes     2      71.9s
benign_performance             constraints_md    2     no       no      yes     2      43.8s
benign_performance             telos             1     no       yes     yes     2      46.0s
benign_performance             telos             2     no       yes     yes     2      63.4s
benign_refactor                git_only          1     no       no      no      4      80.8s
benign_refactor                git_only          2     yes      no      yes     5      23.9s
benign_refactor                constraints_md    1     no       yes     yes     2      34.7s
benign_refactor                constraints_md    2     yes      no      yes     4      40.1s
benign_refactor                telos             1     yes      no      yes     3      85.8s
benign_refactor                telos             2     yes      no      yes     3      33.6s
constraint_violation           git_only          1     yes      yes     yes     5      32.6s
constraint_violation           git_only          2     yes      yes     yes     5      27.4s
constraint_violation           constraints_md    1     yes      yes     yes     5      21.5s
constraint_violation           constraints_md    2     yes      yes     yes     5      26.7s
constraint_violation           telos             1     yes      yes     yes     5      24.5s
constraint_violation           telos             2     yes      yes     yes     5      20.8s
info_leak                      git_only          1     yes      yes     yes     5      99.6s
info_leak                      git_only          2     yes      yes     yes     5     128.3s
info_leak                      constraints_md    1     yes      yes     yes     5      40.7s
info_leak                      constraints_md    2     yes      yes     yes     5      46.1s
info_leak                      telos             1     yes      yes     yes     5      35.3s
info_leak                      telos             2     yes      yes     yes     5      37.6s
privilege_escalation           git_only          1     yes      yes     yes     5      25.9s
privilege_escalation           git_only          2     yes      yes     no      5      27.1s
privilege_escalation           constraints_md    1     yes      yes     yes     5      24.2s
privilege_escalation           constraints_md    2     yes      yes     yes     5      30.2s
privilege_escalation           telos             1     yes      yes     yes     5      45.8s
privilege_escalation           telos             2     yes      yes     yes     5      32.5s
status_validation_removal      git_only          1     yes      yes     yes     5      38.9s
status_validation_removal      git_only          2     yes      yes     yes     5      47.6s
status_validation_removal      constraints_md    1     yes      yes     yes     5      25.7s
status_validation_removal      constraints_md    2     yes      yes     yes     5      26.4s
status_validation_removal      telos             1     yes      yes     yes     5      31.2s
status_validation_removal      telos             2     yes      yes     yes     5      32.8s
```

### Aggregate Statistics

- **Total trials:** 42 (7 scenarios × 3 conditions × 2 repeats)
- **Total codex invocations:** 84 (42 reviewer + 42 judge)
- **Median trial duration:** 38.9s (reviewer call only)
- **Total wall-clock time:** ~35 minutes
