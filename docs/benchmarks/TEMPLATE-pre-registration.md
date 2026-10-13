# Benchmark pre-registration — <run name>

Copy this file, fill every field BEFORE running the benchmark, and link it
from the change the run gates. The numbers themselves go into a NEW dated
run file afterwards, as usual — this file is the contract, not the result.

## Run identity

- Date (UTC):
- Dataset + version + sha256:
- Commit under test (full sha):
- Hardware (cpu / ram / os):
- Mode (local embeddings | zero-llm | other):

## Hypothesis (declared before any number exists)

One or two sentences: what change is being measured, and what outcome is
expected if it works.

## Primary metric and regression threshold

- Primary metric (the one number this run gates on):
- Regression threshold for this run: <[N]% or absolute points>. A published
  slice that drops beyond this, and keeps dropping on a confirmation re-run,
  is a regression to fix, not a note to publish.
- Secondary metrics (reported, not gated):

## Kill criteria

What makes this run invalid or the change abandoned before it ships — for
example: the baseline-vs-baseline determinism check not clean, a dataset
hash mismatch, or a threshold breach with no explanation accepted before the
next release. Written now, not negotiated after the numbers arrive.

## Amendment policy

A published run file is immutable. A correction, a re-run, or a revised
expectation is a NEW dated file that links the one it amends. Nothing here
retroactively rewrites an already-published number.

## Result (filled after the run)

- Run file:
- Outcome vs. threshold:
- Notes:
