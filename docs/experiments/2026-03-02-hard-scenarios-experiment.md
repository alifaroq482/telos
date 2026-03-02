# Hard Scenarios Experiment Report

**Date:** 2026-03-02
**Framework:** telos-experiment v0.1.0
**Agent:** codex-cli 0.96.0 (agent mode via `codex exec`, model gpt-5.2)
**Repeats:** N=5 per condition per scenario
**Conditions:** Git-only, CONSTRAINTS.md, Telos JSON

---

## 1. Summary

Follow-up to the 2026-03-01 baseline experiment, which found all 4 existing true-positive scenarios detected at 100% across all conditions — including git-only. The baseline violations were too obvious from diffs alone (numeric `3600->86400`, hardcoded `UserRole::Admin`, `user_id` in error strings, deleted `validate_transition()`).

This experiment introduces 4 new scenarios designed along **four complexity dimensions**:
- **Multi-file changes** (2-3 files per scenario)
- **Large diffs** (80-110 lines of changes)
- **Multi-constraint interaction** (multiple constraints in play, some satisfied, some violated)
- **Business logic reasoning chains** (3-4 step reasoning needed to connect diff to constraint violation)

**Key finding:** Two of four hard scenarios show strong differentiation between git-only and context-augmented conditions. The `board_archival_cascade` scenario catches the violation only 40% of the time in git-only (vs 100% with context), and `rate_limit_config_migration` catches it 60% of the time. Neither scenario ever cites the constraint in git-only (0%), vs 100% with both CONSTRAINTS.md and Telos.

---

## 2. Scenarios

| # | Scenario | Files | Diff Lines | Violated Constraints | Reasoning Steps |
|---|----------|-------|-----------|---------------------|-----------------|
| 1 | multi_file_payment_refactor | 3 | ~100 | 1 of 4 | 4 (lock removal -> TOCTOU -> race -> double-spend) |
| 2 | board_archival_cascade | 2 | ~90 | 2 of 5 | 4 (archive -> remove from map -> tasks orphaned -> constraint) |
| 3 | rate_limit_config_migration | 2 | ~80 | 1 of 4 | 4 (default -> 100rpm -> decision was 10rpm -> credential stuffing) |
| 4 | notification_pipeline_optimization | 3 | ~110 | 3 of 5 | 3x2 (each violation + their interaction) |

### Why each scenario is designed to be hard

**multi_file_payment_refactor**: A payment handler drops a `Mutex` lock guard. The refactor introduces a `batch_write()` method in `ledger.rs` which has its own internal locking — so the code looks correct ("we're using batch_write, which locks internally"). But the *balance check* now happens outside any lock, creating a TOCTOU race. The violation requires cross-file reasoning: handler.rs (lock removed) + ledger.rs (batch_write locks internally) + understanding that the gap *between* get_balance and batch_write is the problem.

**board_archival_cascade**: Board archival is implemented with proper auth checks, validation, and audit logging. The code is locally correct — it does everything right for the *board*. But tasks reference boards by `board_id`, and the archival removes the board from the active `boards` HashMap without reassigning tasks. This creates orphaned tasks invisible in the UI. The violation requires knowing the cross-module constraint about task-board referential integrity.

**rate_limit_config_migration**: A well-engineered migration from hardcoded rate limits to a configurable struct. The code is clean, documented, and follows patterns. But `RateLimitConfig::default()` sets `login_rpm: 100`, while a recorded project decision set login to `10 rpm` with detailed credential-stuffing rationale. 100 rpm looks reasonable without context — the violation is only apparent when you know the specific policy decision.

**notification_pipeline_optimization**: Three legitimate performance patterns (priority queue, concurrent dispatch, bloom filter dedup) that each individually look correct but together break FIFO ordering, introduce false-positive dedup, and can lose notifications entirely through their interaction.

---

## 3. Results

### 3.1 Aggregate Results

| Scenario | Metric | Git-only | CONST.md | Telos |
|----------|--------|----------|----------|-------|
| **board_archival_cascade** | Caught | **40%** | 100% | 100% |
| | Rejected | 100% | 100% | 100% |
| | Cited constraint | **0%** | 100% | 100% |
| | Reasoning quality | 2.6 | 4.8 | 5.0 |
| **rate_limit_config_migration** | Caught | **60%** | 100% | 100% |
| | Rejected | 100% | 100% | 100% |
| | Cited constraint | **0%** | 100% | 100% |
| | Reasoning quality | 3.0 | 4.8 | 5.0 |
| **notification_pipeline_optimization** | Caught | **80%** | 100% | 100% |
| | Rejected | 80% | 100% | 100% |
| | Cited constraint | **40%** | 100% | 100% |
| | Reasoning quality | 3.4 | 4.8 | 4.8 |
| **multi_file_payment_refactor** | Caught | 100% | 100% | 100% |
| | Rejected | 100% | 100% | 100% |
| | Cited constraint | 100% | 100% | 100% |
| | Reasoning quality | 4.6 | 5.0 | 5.0 |

### 3.2 Comparison with Baseline Scenarios

| Scenario Type | Git-only Caught | Git-only Cited | CONST.md Caught | Telos Caught |
|---------------|:-:|:-:|:-:|:-:|
| **Baseline avg** (4 scenarios, N=2) | 100% | 88% | 100% | 100% |
| **Hard avg** (4 scenarios, N=5) | **70%** | **35%** | 100% | 100% |
| Delta | **-30pp** | **-53pp** | 0 | 0 |

The hard scenarios successfully differentiate git-only from context-augmented conditions. Context conditions remain at 100% detection and citation.

### 3.3 Condition Comparison: CONSTRAINTS.md vs Telos

Both CONSTRAINTS.md and Telos achieve 100% on all metrics across all hard scenarios. The differentiation between them, if any, appears in reasoning quality:

| Scenario | CONST.md Quality | Telos Quality | Delta |
|----------|:---:|:---:|:---:|
| board_archival_cascade | 4.8 | **5.0** | +0.2 |
| rate_limit_config_migration | 4.8 | **5.0** | +0.2 |
| notification_pipeline_optimization | 4.8 | 4.8 | 0.0 |
| multi_file_payment_refactor | 5.0 | 5.0 | 0.0 |

At N=5, the quality differences are not statistically significant. Telos and CONSTRAINTS.md perform equivalently on these scenarios.

---

## 4. Analysis

### 4.1 Two Scenarios Show Strong Differentiation

**board_archival_cascade** (40% git-only caught, 0% cited) and **rate_limit_config_migration** (60% caught, 0% cited) validate the hypothesis that certain violation types are genuinely invisible from diffs alone:

- **Cross-module referential integrity** (board_archival_cascade): The code is locally correct in every file. The orphan problem only emerges from knowing the task-board relationship constraint. Without context, the agent sometimes speculates about "possible missing cleanup" but cannot ground it in a specific constraint, hence 0% cited.

- **Policy value violations** (rate_limit_config_migration): The `login_rpm: 100` default looks like a reasonable number. Without knowing the recorded decision for `10 rpm`, the agent identifies the rate limit as "perhaps too permissive" in some trials but never cites the specific policy or quantifies the credential-stuffing impact.

### 4.2 One Scenario Is Partially Harder

**notification_pipeline_optimization** (80% caught, 40% cited in git-only) shows partial differentiation. The violations (bloom filter false positives, broken FIFO ordering) are detectable from the diff because they involve well-known antipatterns. However, the agent sometimes misses the interaction between the violations and fails to cite the specific constraint.

### 4.3 One Scenario Remains Too Easy

**multi_file_payment_refactor** (100% caught, 100% cited in git-only) did not differentiate. The TOCTOU/lock-removal pattern is a well-known concurrency antipattern that the agent detects from the diff alone. The removal of `let _guard = self.ledger_lock.lock()` is a strong signal.

### 4.4 Rejection Rate vs Caught Rate

Interestingly, `board_archival_cascade` has 100% rejection rate in git-only despite only 40% caught rate. The agent finds *other* reasons to reject (missing tests, incomplete error handling, etc.) even when it misses the core orphan violation. This suggests "rejected" alone is an insufficient metric — **"caught the specific constraint violation"** is the meaningful signal.

### 4.5 Reasoning Quality Correlates with Context

Average reasoning quality by condition:

| Condition | Avg Quality |
|-----------|:-----------:|
| Git-only | 3.4 |
| CONSTRAINTS.md | 4.9 |
| Telos | 5.0 |

Context doesn't just help detection — it improves the quality of reasoning. Git-only reviews tend to be speculative ("this might cause issues"), while context-augmented reviews are grounded ("this violates the recorded constraint that...").

---

## 5. Per-Trial Detail

```
Scenario                                   Condition          Trial  Caught  Reject  Cited  Quality  Duration
--------------------------------------------------------------------------------------------------------------
board_archival_cascade                     git_only               1     yes     yes     no        3     69.8s
board_archival_cascade                     git_only               2     yes     yes     no        4     55.6s
board_archival_cascade                     git_only               3      no     yes     no        2     78.6s
board_archival_cascade                     git_only               4      no     yes     no        2     62.4s
board_archival_cascade                     git_only               5      no     yes     no        2     74.5s
board_archival_cascade                     constraints_md         1     yes     yes    yes        5     52.4s
board_archival_cascade                     constraints_md         2     yes     yes    yes        5     79.1s
board_archival_cascade                     constraints_md         3     yes     yes    yes        5     76.0s
board_archival_cascade                     constraints_md         4     yes     yes    yes        4     55.9s
board_archival_cascade                     constraints_md         5     yes     yes    yes        5     57.6s
board_archival_cascade                     telos                  1     yes     yes    yes        5     65.6s
board_archival_cascade                     telos                  2     yes     yes    yes        5    133.2s
board_archival_cascade                     telos                  3     yes     yes    yes        5     37.9s
board_archival_cascade                     telos                  4     yes     yes    yes        5    107.7s
board_archival_cascade                     telos                  5     yes     yes    yes        5    148.7s
multi_file_payment_refactor                git_only               1     yes     yes    yes        5    109.2s
multi_file_payment_refactor                git_only               2     yes     yes    yes        5     78.6s
multi_file_payment_refactor                git_only               3     yes     yes    yes        5    115.4s
multi_file_payment_refactor                git_only               4     yes     yes    yes        3     88.1s
multi_file_payment_refactor                git_only               5     yes     yes    yes        5    111.3s
multi_file_payment_refactor                constraints_md         1     yes     yes    yes        5     95.5s
multi_file_payment_refactor                constraints_md         2     yes     yes    yes        5     84.3s
multi_file_payment_refactor                constraints_md         3     yes     yes    yes        5     79.0s
multi_file_payment_refactor                constraints_md         4     yes     yes    yes        5    119.7s
multi_file_payment_refactor                constraints_md         5     yes     yes    yes        5    132.1s
multi_file_payment_refactor                telos                  1     yes     yes    yes        5     65.2s
multi_file_payment_refactor                telos                  2     yes     yes    yes        5     94.6s
multi_file_payment_refactor                telos                  3     yes     yes    yes        5     81.5s
multi_file_payment_refactor                telos                  4     yes     yes    yes        5    114.3s
multi_file_payment_refactor                telos                  5     yes     yes    yes        5    132.1s
notification_pipeline_optimization         git_only               1     yes     yes     no        4    164.2s
notification_pipeline_optimization         git_only               2     yes     yes    yes        5     76.3s
notification_pipeline_optimization         git_only               3     yes     yes    yes        4    261.6s
notification_pipeline_optimization         git_only               4      no      no     no        ?    161.0s
notification_pipeline_optimization         git_only               5     yes     yes     no        4    209.7s
notification_pipeline_optimization         constraints_md         1     yes     yes    yes        5     68.6s
notification_pipeline_optimization         constraints_md         2     yes     yes    yes        5     93.7s
notification_pipeline_optimization         constraints_md         3     yes     yes    yes        5    102.0s
notification_pipeline_optimization         constraints_md         4     yes     yes    yes        5     80.9s
notification_pipeline_optimization         constraints_md         5     yes     yes    yes        4     90.5s
notification_pipeline_optimization         telos                  1     yes     yes    yes        5    108.3s
notification_pipeline_optimization         telos                  2     yes     yes    yes        5     90.2s
notification_pipeline_optimization         telos                  3     yes     yes    yes        4     98.7s
notification_pipeline_optimization         telos                  4     yes     yes    yes        5    133.6s
notification_pipeline_optimization         telos                  5     yes     yes    yes        5    105.5s
rate_limit_config_migration                git_only               1      no     yes     no        3     68.7s
rate_limit_config_migration                git_only               2     yes     yes     no        3     72.1s
rate_limit_config_migration                git_only               3     yes     yes     no        3     68.7s
rate_limit_config_migration                git_only               4     yes     yes     no        3     66.4s
rate_limit_config_migration                git_only               5      no     yes     no        3     62.0s
rate_limit_config_migration                constraints_md         1     yes     yes    yes        5     55.0s
rate_limit_config_migration                constraints_md         2     yes     yes    yes        5     75.8s
rate_limit_config_migration                constraints_md         3     yes     yes    yes        5     64.6s
rate_limit_config_migration                constraints_md         4     yes     yes    yes        4     46.8s
rate_limit_config_migration                constraints_md         5     yes     yes    yes        5     67.7s
rate_limit_config_migration                telos                  1     yes     yes    yes        5     63.6s
rate_limit_config_migration                telos                  2     yes     yes    yes        5     64.1s
rate_limit_config_migration                telos                  3     yes     yes    yes        5     72.7s
rate_limit_config_migration                telos                  4     yes     yes    yes        5    127.1s
rate_limit_config_migration                telos                  5     yes     yes    yes        5     75.5s
```

---

## 6. Limitations

1. **N=5 is still low** — Need N>=10 per cell for statistical significance, N>=30 for confidence intervals. Current results are directional.
2. **Single model** — Only tested with codex (gpt-5.2 via sub2api). Results may differ with Claude, o3, or other models.
3. **API instability** — The `sub2api` provider had intermittent stream disconnections. `rate_limit_config_migration` failed 4 times before completing. This adds variance from non-random retry patterns.
4. **No CONSTRAINTS.md vs Telos differentiation** — Both context conditions achieve identical results. Either the scenarios need further refinement to expose Telos's structured advantage, or the current constraint density is sufficient for flat markdown to work.
5. **LLM-as-judge consistency** — The judge (also gpt-5.2) may score inconsistently across runs.
6. **multi_file_payment_refactor too easy** — Lock removal is a well-known pattern. Need scenarios where the code pattern alone doesn't signal the violation.

---

## 7. Next Steps

### Immediate
1. **Increase N to 10+** for the two differentiating scenarios (board_archival_cascade, rate_limit_config_migration)
2. **Add retry logic** to the experiment runner to handle transient API failures
3. **Design scenarios that differentiate CONSTRAINTS.md from Telos** — Current results show no difference; need scenarios where decision rationale (why the constraint exists, what alternatives were rejected) is essential for detection

### Scenario Design Insights
4. **Best violation type for differentiation**: Cross-module referential integrity and policy value violations. These are invisible from diffs alone.
5. **Worst violation type for differentiation**: Concurrency antipatterns (lock removal, race conditions). These are well-known patterns detectable from diffs.
6. **Middle ground**: Performance optimizations that break semantic guarantees (notification_pipeline_optimization). Partially detectable.

### Methodology
7. **Multi-model comparison** — Test with Claude Opus, Claude Sonnet alongside codex
8. **Add false-positive hard scenarios** — Benign changes that look like they might violate constraints but don't
9. **Statistical testing** — Fisher's exact test or chi-squared on caught/not-caught contingency tables

---

## 8. Raw Data

- Combined results: `docs/experiments/2026-03-02-hard-scenarios-results.json`
- Individual runs: `.telos-experiment/results/run-20260302-*.json`
- Total trials: 60 (4 scenarios x 3 conditions x 5 repeats)
- Total codex invocations: 120 (60 reviewer + 60 judge)
- Median trial duration: 80.9s
