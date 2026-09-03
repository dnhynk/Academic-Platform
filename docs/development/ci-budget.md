# Hosted CI budget record

This record makes timeout headroom a number rather than a qualitative claim.
Elapsed time is GitHub's job `completed_at - started_at`; utilization is
`elapsed_seconds / (timeout-minutes * 60)`. Queue time is excluded. The source
is the Actions jobs API, not a duration copied from the web UI.

## Refresh rule

Refresh the latest-run table after a completed hosted run whenever a change
touches `.github/workflows/ci.yml`, workspace membership, a default-workspace
test, or one of the non-default Rust feature lanes. Also refresh it when any
required job reaches 80% of its timeout or varies by at least 20% from the
recorded elapsed time. A row at or above 80% is a review trigger for another
split, an admitted cache, or a measured timeout change; it is not permission to
remove a verification command.

Use `gh api repos/dnhynk/Academic-Platform/actions/runs/<run-id>/jobs?per_page=100`
and retain, for every job, `name`, `conclusion`, `started_at`, and
`completed_at`. For reruns, query
`actions/runs/<run-id>/attempts/<attempt>/jobs?per_page=100` so the first timed
out attempt is not overwritten by its retry. Step attribution comes from the
same response's `steps[].started_at` and `steps[].completed_at`; use the job log
timestamps only to separate compilation, test execution, and doc tests inside
one Cargo step.

## Pre-split evidence

Run [33668524442](https://github.com/dnhynk/Academic-Platform/actions/runs/33668524442)
at `f7ab1c8dfbc9b70a1111cab6493047f538c518fe` completed 12/12 before the split.
It is the measurement that triggered this change.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:04 | 5:00 | 1.3% |
| `rust-ubuntu-latest` | 7:21 | 20:00 | 36.8% |
| `rust-ubuntu-24.04-arm` | 6:15 | 20:00 | 31.3% |
| `rust-windows-latest` | 19:28 | 20:00 | 97.3% |
| `rust-windows-11-arm` | 15:43 | 20:00 | 78.6% |
| `rust-macos-latest` | 7:43 | 20:00 | 38.6% |
| `phase1-exit-ubuntu-latest` | 4:09 | 45:00 | 9.2% |
| `phase1-exit-windows-latest` | 7:55 | 45:00 | 17.6% |
| `encrypted-store-lane-ubuntu-latest` | 3:21 | 45:00 | 7.4% |
| `encrypted-portability-lane-ubuntu-latest` | 3:48 | 45:00 | 8.4% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:48 | 45:00 | 15.1% |
| `pnpm-contracts` | 0:49 | 15:00 | 5.4% |

The five Rust jobs break down as follows. `Workspace test` is the single
`cargo test --workspace --locked` step and includes its doc tests. There is no
standalone `cargo build` step; compilation occurs inside clippy and test. Times
may differ from the job total by a few seconds because the API timestamps have
one-second resolution and GitHub has step transitions.

| Label | Job | Setup + fmt | Default clippy | Workspace test + doc tests | Feature clippy | Feature tests | Fixture CLI | Post |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `ubuntu-latest` | 7:21 | 0:17 | 0:38 | 3:23 | 0:14 | 2:24 | 0:19 | 0:04 |
| `ubuntu-24.04-arm` | 6:15 | 0:17 | 0:32 | 2:59 | 0:16 | 1:45 | 0:19 | 0:04 |
| `windows-latest` | 19:28 | 1:13 | 1:00 | 11:11 | 0:24 | 3:58 | 0:43 | 0:56 |
| `windows-11-arm` | 15:43 | 1:10 | 0:56 | 8:47 | 0:28 | 3:28 | 0:38 | 0:13 |
| `macos-latest` | 7:43 | 0:26 | 0:56 | 3:42 | 0:21 | 1:46 | 0:21 | 0:09 |

For `rust-windows-latest`, the job log further places 2:40 of the 11:11
workspace-test step in Cargo compilation, about 8:26 before the first doc-test
line, and about 0:04 in doc tests. The timed-out first attempt of run
[33645176408](https://github.com/dnhynk/Academic-Platform/actions/runs/33645176408)
lasted 20:03; rerunning that same commit lasted 16:06, a 3:57 runner spread.

## Post-split budget

The workflow now materializes 17 required jobs. The five `rust-default-*` jobs
retain formatting, default clippy, the workspace test (including doc tests),
and all fixture commands. The five `rust-features-*` jobs retain every
encrypted-object, rotation/retention, transcript, and native-worker clippy/test
command on the same five labels. Both groups have a 30-minute limit.

The latest run,
[33698230519](https://github.com/dnhynk/Academic-Platform/actions/runs/33698230519),
completed 17/17 at `8ac8f17` — `P2-RF11`, which changed five default-workspace
tests and two pnpm source scans.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:04 | 5:00 | 1.3% |
| `rust-default-ubuntu-latest` | 4:36 | 30:00 | 15.3% |
| `rust-default-ubuntu-24.04-arm` | 3:51 | 30:00 | 12.8% |
| `rust-default-windows-latest` | 12:16 | 30:00 | 40.9% |
| `rust-default-windows-11-arm` | 12:49 | 30:00 | 42.7% |
| `rust-default-macos-latest` | 4:13 | 30:00 | 14.1% |
| `rust-features-ubuntu-latest` | 2:40 | 30:00 | 8.9% |
| `rust-features-ubuntu-24.04-arm` | 2:20 | 30:00 | 7.8% |
| `rust-features-windows-latest` | 5:20 | 30:00 | 17.8% |
| `rust-features-windows-11-arm` | 4:41 | 30:00 | 15.6% |
| `rust-features-macos-latest` | 2:13 | 30:00 | 7.4% |
| `phase1-exit-ubuntu-latest` | 4:05 | 45:00 | 9.1% |
| `phase1-exit-windows-latest` | 7:58 | 45:00 | 17.7% |
| `encrypted-store-lane-ubuntu-latest` | 3:06 | 45:00 | 6.9% |
| `encrypted-portability-lane-ubuntu-latest` | 5:07 | 45:00 | 11.4% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:50 | 45:00 | 15.2% |
| `pnpm-contracts` | 0:52 | 15:00 | 5.8% |

The slowest job is now `rust-default-windows-latest` at 67.7%. Its 20:18
also shows that splitting alone while retaining the old 20-minute timeout
would still have failed on this runner. The independently completed feature
job used 18.1% instead of extending that default job by another 5:26. Windows
ARM is the next-highest Rust default at 43.0%; every Linux, Linux ARM, and macOS
Rust job is at or below 16.3%.

## The `P2-G6` run

`P2-G6` adds one workspace member, `academic-consent`, which the refresh rule
above names as a trigger. Run
[33696263675](https://github.com/dnhynk/Academic-Platform/actions/runs/33696263675)
at `f0a4ddef28beaab00c20a360a5adb1a255855135` completed 17/17.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:03 | 5:00 | 1.0% |
| `rust-default-ubuntu-latest` | 4:11 | 30:00 | 13.9% |
| `rust-default-ubuntu-24.04-arm` | 4:08 | 30:00 | 13.8% |
| `rust-default-windows-latest` | 14:08 | 30:00 | 47.1% |
| `rust-default-windows-11-arm` | 13:32 | 30:00 | 45.1% |
| `rust-default-macos-latest` | 5:17 | 30:00 | 17.6% |
| `rust-features-ubuntu-latest` | 2:36 | 30:00 | 8.7% |
| `rust-features-ubuntu-24.04-arm` | 2:15 | 30:00 | 7.5% |
| `rust-features-windows-latest` | 5:10 | 30:00 | 17.2% |
| `rust-features-windows-11-arm` | 4:42 | 30:00 | 15.7% |
| `rust-features-macos-latest` | 1:54 | 30:00 | 6.3% |
| `phase1-exit-ubuntu-latest` | 4:31 | 45:00 | 10.0% |
| `phase1-exit-windows-latest` | 7:59 | 45:00 | 17.7% |
| `encrypted-store-lane-ubuntu-latest` | 3:21 | 45:00 | 7.4% |
| `encrypted-portability-lane-ubuntu-latest` | 4:49 | 45:00 | 10.7% |
| `rotation-orchestration-lane-ubuntu-latest` | 5:12 | 45:00 | 11.6% |
| `pnpm-contracts` | 0:51 | 15:00 | 5.7% |

The slowest job is still `rust-default-windows-latest`, at 47.1% against the
67.7% the post-split table recorded — a 6:10 difference on the same workflow with
one more workspace member, which is runner spread rather than an effect of this
change. That is the 3:57 spread the pre-split section already recorded on the
same label, and it is why this table is refreshed rather than compared: the
number that matters is the headroom, and every job on this run is at or below
47.1%.

The new member adds one crate to `cargo clippy --workspace --all-targets` and
one test binary plus a `trybuild` case to `cargo test --workspace`. The
`trybuild` case compiles two small programs and is the one addition that costs
more than a compile of the crate itself; it sits inside the workspace test step
on the `rust-default-*` jobs.

`encrypted-store-lane-ubuntu-latest` is the job migration `0006` changes, and it
is at 7.4%.

## The `P2-RF11` run

`P2-RF11` changes five default-workspace tests and two pnpm source scans, which
the refresh rule names as a trigger, and `rust-default-windows-latest` moved by
more than the 20% the rule names as a second one. Run
[33698230519](https://github.com/dnhynk/Academic-Platform/actions/runs/33698230519)
completed 17/17; the docs-only follow-up
[33700472647](https://github.com/dnhynk/Academic-Platform/actions/runs/33700472647)
did too.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:04 | 5:00 | 1.3% |
| `rust-default-ubuntu-latest` | 4:36 | 30:00 | 15.3% |
| `rust-default-ubuntu-24.04-arm` | 3:51 | 30:00 | 12.8% |
| `rust-default-windows-latest` | 12:16 | 30:00 | 40.9% |
| `rust-default-windows-11-arm` | 12:49 | 30:00 | 42.7% |
| `rust-default-macos-latest` | 4:13 | 30:00 | 14.1% |
| `rust-features-ubuntu-latest` | 2:40 | 30:00 | 8.9% |
| `rust-features-ubuntu-24.04-arm` | 2:20 | 30:00 | 7.8% |
| `rust-features-windows-latest` | 5:20 | 30:00 | 17.8% |
| `rust-features-windows-11-arm` | 4:41 | 30:00 | 15.6% |
| `rust-features-macos-latest` | 2:13 | 30:00 | 7.4% |
| `phase1-exit-ubuntu-latest` | 4:05 | 45:00 | 9.1% |
| `phase1-exit-windows-latest` | 7:58 | 45:00 | 17.7% |
| `encrypted-store-lane-ubuntu-latest` | 3:06 | 45:00 | 6.9% |
| `encrypted-portability-lane-ubuntu-latest` | 5:07 | 45:00 | 11.4% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:50 | 45:00 | 15.2% |
| `pnpm-contracts` | 0:52 | 15:00 | 5.8% |

The slowest job on this run is `rust-default-windows-11-arm` at 42.7%, with
`rust-default-windows-latest` beside it at 40.9%. Every other job is at or below
17.8%, and every Linux, Linux ARM and macOS Rust job is at or below 15.3%.

**`rust-default-windows-latest` now has three readings on one workflow: 20:18,
14:08 and 12:16 — 67.7%, 47.1% and 40.9%.** The section above already read the
second against the first as runner spread. The third confirms it and settles the
shape: this job's elapsed time varies by about 40% between runs with no
repository change that explains it, so no single reading of it is a budget. Size
headroom off the worst of the three, 67.7%, and treat a fourth reading as
evidence about the *range* rather than as a replacement for the last one. The
three tables are kept for that reason.


## The `P2-M1` run

`P2-M1` adds one workspace member, `academic-model-run`, and a canonical-store
migration, both of which the refresh rule names as triggers. Run
[33704083224](https://github.com/dnhynk/Academic-Platform/actions/runs/33704083224)
at `3c03e018a31efee4b416b1baa8f17c8c9642f05c` completed 17/17.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:05 | 5:00 | 1.7% |
| `rust-default-ubuntu-latest` | 4:57 | 30:00 | 16.5% |
| `rust-default-ubuntu-24.04-arm` | 4:15 | 30:00 | 14.2% |
| `rust-default-windows-latest` | 15:28 | 30:00 | 51.6% |
| `rust-default-windows-11-arm` | 11:57 | 30:00 | 39.8% |
| `rust-default-macos-latest` | 6:19 | 30:00 | 21.1% |
| `rust-features-ubuntu-latest` | 2:27 | 30:00 | 8.2% |
| `rust-features-ubuntu-24.04-arm` | 2:23 | 30:00 | 7.9% |
| `rust-features-windows-latest` | 5:42 | 30:00 | 19.0% |
| `rust-features-windows-11-arm` | 4:57 | 30:00 | 16.5% |
| `rust-features-macos-latest` | 1:58 | 30:00 | 6.6% |
| `phase1-exit-ubuntu-latest` | 4:07 | 45:00 | 9.1% |
| `phase1-exit-windows-latest` | 10:48 | 45:00 | 24.0% |
| `encrypted-store-lane-ubuntu-latest` | 3:28 | 45:00 | 7.7% |
| `encrypted-portability-lane-ubuntu-latest` | 4:30 | 45:00 | 10.0% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:56 | 45:00 | 15.4% |
| `pnpm-contracts` | 0:42 | 15:00 | 4.7% |

The slowest job is `rust-default-windows-latest` at 51.6%, against 40.9% on the
`P2-RF11` run. That is a 3:12 difference on one label, and this page has already
recorded a 3:57 spread on the same label for the *same commit*, so the movement
is inside the noise this repository has measured and is not attributable to the
new member. What the table establishes is the headroom: every job is at or below
51.6%, and nothing is near the 80% review trigger.

`phase1-exit-windows-latest` is the second-largest mover, 7:59 to 10:48. It
links the whole workspace under the fault-injection feature set, so a new member
is compiled there twice; at 24.0% of a 45-minute limit it is recorded rather
than acted on.

`encrypted-store-lane-ubuntu-latest` is the job migration `0007` changes, and it
is at 7.7%. That lane is also the only one that pins `STORE_MIGRATION_SQL` as a
whole, so it is where a migration added to that set is caught: it failed on an
earlier commit of this branch, at `len() == 5` against a six-element set, before
the pin was extended.

## The `P2-L1` run

`P2-L1` adds one workspace member, `academic-capture-gate`, and two commands to
the `rust-features` job — a clippy and a test of its non-default
`native-capture` lane. Both are triggers the refresh rule names: workspace
membership, and one of the non-default Rust feature lanes. The job count is
unchanged at **17**; the new lane is two steps inside a job that already exists,
not a sixth feature job.

Run
[33710769279](https://github.com/dnhynk/Academic-Platform/actions/runs/33710769279)
completed **17/17** on `6c02ba4`.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:05 | 5:00 | 1.7% |
| `rust-default-ubuntu-latest` | 5:10 | 30:00 | 17.2% |
| `rust-default-ubuntu-24.04-arm` | 4:23 | 30:00 | 14.6% |
| `rust-default-windows-latest` | 13:47 | 30:00 | 45.9% |
| `rust-default-windows-11-arm` | 15:26 | 30:00 | 51.4% |
| `rust-default-macos-latest` | 6:07 | 30:00 | 20.4% |
| `rust-features-ubuntu-latest` | 3:46 | 30:00 | 12.6% |
| `rust-features-ubuntu-24.04-arm` | 2:43 | 30:00 | 9.1% |
| `rust-features-windows-latest` | 6:11 | 30:00 | 20.6% |
| `rust-features-windows-11-arm` | 6:14 | 30:00 | 20.8% |
| `rust-features-macos-latest` | 2:25 | 30:00 | 8.1% |
| `phase1-exit-ubuntu-latest` | 4:18 | 45:00 | 9.6% |
| `phase1-exit-windows-latest` | 10:00 | 45:00 | 22.2% |
| `encrypted-store-lane-ubuntu-latest` | 3:24 | 45:00 | 7.6% |
| `encrypted-portability-lane-ubuntu-latest` | 5:09 | 45:00 | 11.4% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:40 | 45:00 | 14.8% |
| `pnpm-contracts` | 0:56 | 15:00 | 6.2% |

**The two new steps cost about a minute.** Every `rust-features` job moved up
against the `P2-M1` reading, the run immediately before this one —
`ubuntu-latest` 2:27 → 3:46, `ubuntu-24.04-arm` 2:23 → 2:43, `windows-latest`
5:42 → 6:11, `windows-11-arm` 4:57 → 6:14, `macos-latest` 1:58 → 2:25. That is
+0:20 to +1:19, and the group's worst utilization is 20.8%, up from 19.0%, with
30 minutes of limit against it.

**`rust-default-windows-11-arm` is the slowest job at 51.4%**, and the Windows
default lane keeps the spread this page has recorded four times: 20:18, 14:08,
12:16 and 15:28 for `windows-latest`, and 12:49, 11:57 and 15:26 for
`windows-11-arm`. This run reads 13:47 and 15:26, both inside that range, and
`windows-latest` is *faster* here than on the run before it. Nothing here is
evidence of a new cost: the default lane gained one workspace member whose
default-feature test tree is 20 tests that finish in well under a second
locally, and the two feature-lane steps are in a different job. Size headroom
off the worst reading on this workflow, 67.7%, rather than off any single run.

The one job that failed on the first attempt of this branch is recorded in the
section below, because it is a compilation gate rather than a budget reading.

### The macOS feature lane, first attempt

Run
[33709678178](https://github.com/dnhynk/Academic-Platform/actions/runs/33709678178)
on `927c5f5` completed **16/17**: `rust-features-macos-latest` failed on
`Lint the capture device lane`. That is not a timeout and not a test result. The
capture gate's native suite was gated on the `native-capture` feature alone, so
it compiled on a target that has neither of the two backends and its
per-platform helpers had no arm to return from. Gating it on the feature *and* a
target with a backend — which is how `academic-worker`'s containment suite is
gated — is `6c02ba4`, and the rerun above is green.

It is recorded here because the lesson is the same one this page's Windows
section carries in the other direction: **a lane that fails needs its cause
named before its elapsed time means anything.** A per-platform suite that
compiles on a third platform is a compile gate, and no rerun of the same commit
would have changed it.

## The `P2-M2` run

`P2-M2` adds one workspace member, `academic-proposal`, and a canonical-store
migration, `0009`. Both are triggers the refresh rule names. The job count is
unchanged at **17**: the new member has no non-default feature lane, so it adds
no job and no step, only test targets inside the default lane and one more
migration for the encrypted-store lane to apply.

Run
[33715585336](https://github.com/dnhynk/Academic-Platform/actions/runs/33715585336)
completed **17/17** on `5dfbd46`, with `rust-features-windows-latest` green on
its second attempt of the same commit — see the launch-error section below,
which is what that rerun falsifies.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:09 | 5:00 | 3.0% |
| `rust-default-ubuntu-latest` | 5:12 | 30:00 | 17.3% |
| `rust-default-ubuntu-24.04-arm` | 4:30 | 30:00 | 15.0% |
| `rust-default-windows-latest` | 14:55 | 30:00 | 49.7% |
| `rust-default-windows-11-arm` | 12:29 | 30:00 | 41.6% |
| `rust-default-macos-latest` | 6:52 | 30:00 | 22.9% |
| `rust-features-ubuntu-latest` | 3:08 | 30:00 | 10.4% |
| `rust-features-ubuntu-24.04-arm` | 2:58 | 30:00 | 9.9% |
| `rust-features-windows-latest` | 6:11 | 30:00 | 20.6% |
| `rust-features-windows-11-arm` | 5:38 | 30:00 | 18.8% |
| `rust-features-macos-latest` | 3:46 | 30:00 | 12.6% |
| `phase1-exit-ubuntu-latest` | 4:18 | 45:00 | 9.6% |
| `phase1-exit-windows-latest` | 8:37 | 45:00 | 19.1% |
| `encrypted-store-lane-ubuntu-latest` | 3:12 | 45:00 | 7.1% |
| `encrypted-portability-lane-ubuntu-latest` | 4:55 | 45:00 | 10.9% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:58 | 45:00 | 15.5% |
| `pnpm-contracts` | 0:47 | 15:00 | 5.2% |

**`rust-default-windows-latest` is the slowest job at 49.7%**, up from 45.9% on
the `P2-L1` run. That label's readings on this page are now 20:18, 14:08, 12:16,
15:28, 13:47 and 14:55 — a 8:02 spread across six runs of a workflow whose
content changed only incrementally, and this reading sits in the middle of it.
`windows-11-arm` moved the other way, 15:26 to 12:29, on a run that added a
member to the same lane. Neither direction is attributable to this task: the new
member's default test tree is 26 tests that finish in under a second locally.

**No feature job moved on the new member's account**, and none could: the crate
declares no non-default feature, so the `rust-features` group's five readings
differ from `P2-L1`'s by -0:36 to +1:21 with no step added. The group's worst
utilization is 20.6%.

**`encrypted-store-lane-ubuntu-latest` is the job migration `0009` changes** and
it reads 7.1%, against 7.6% on the run before. That lane is the only one that
pins `STORE_MIGRATION_SQL` as a whole, so it is where a migration added to that
set is caught. It was run locally under WSL2 before every push to this branch,
which is the step `P2-M1` skipped and CI caught for it.

Every job is at or below 49.7%, and nothing is near the 80% review trigger.
Size headroom off the worst reading this page holds for a label, 67.7% on
`rust-default-windows-latest`, rather than off any single run.

## Latest run

`P2-L2` adds one workspace member, `academic-capture`, two steps to the
`rust-features` matrix, and no pnpm package. Workspace membership,
`.github/workflows/ci.yml` and a non-default feature lane are three triggers the
refresh rule names. The job count is unchanged at **17**: the new steps sit
inside the existing feature job and the new crate compiles inside the
`cargo test --workspace --locked` step the five `rust-default-*` jobs already
run.

The table below is run
[33738307227](https://github.com/dnhynk/Academic-Platform/actions/runs/33738307227),
**17/17 on `d97ecab`** — the branch rebased onto `main` after `P2-R1` merged,
which is the head that will merge. It needed no rerun.

Run
[33730639197](https://github.com/dnhynk/Academic-Platform/actions/runs/33730639197)
on the pre-rebase `0e0bff5` also reached 17/17, with two jobs taken there by a
same-commit rerun. Its numbers are not the table's, but its two attempt-1
readings are kept below, because each is a rule this page already holds applied
to a real observation.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:11 | 5:00 | 3.7% |
| `rust-default-ubuntu-latest` | 5:50 | 30:00 | 19.4% |
| `rust-default-ubuntu-24.04-arm` | 4:47 | 30:00 | 15.9% |
| `rust-default-windows-latest` | 16:35 | 30:00 | 55.3% |
| `rust-default-windows-11-arm` | 15:55 | 30:00 | 53.1% |
| `rust-default-macos-latest` | 5:56 | 30:00 | 19.8% |
| `rust-features-ubuntu-latest` | 3:52 | 30:00 | 12.9% |
| `rust-features-ubuntu-24.04-arm` | 3:14 | 30:00 | 10.8% |
| `rust-features-windows-latest` | 5:54 | 30:00 | 19.7% |
| `rust-features-windows-11-arm` | 6:12 | 30:00 | 20.7% |
| `rust-features-macos-latest` | 3:33 | 30:00 | 11.8% |
| `phase1-exit-ubuntu-latest` | 3:56 | 45:00 | 8.7% |
| `phase1-exit-windows-latest` | 8:10 | 45:00 | 18.1% |
| `encrypted-store-lane-ubuntu-latest` | 4:01 | 45:00 | 8.9% |
| `encrypted-portability-lane-ubuntu-latest` | 4:57 | 45:00 | 11.0% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:09 | 45:00 | 13.7% |
| `pnpm-contracts` | 1:05 | 15:00 | 7.2% |

**No job reaches the 80% review trigger on this head**, and
`rust-default-windows-latest` reads 55.3% — inside the range this page already
holds for it. That is the third reading of essentially this tree on that label,
after 30:14 and 18:41 on the pre-rebase head, and it is the strongest evidence
that neither of those was a cost this task added.

### `rust-default-windows-latest` crossed its limit once, on the pre-rebase head

Attempt 1 ran **30:14 against a 30:00 limit — 100.8%, the first reading over
100% this page holds.** It was cancelled, not failed.

`Test Rust workspace` **succeeded** inside it, at 26:27; the cancellation landed
on the next step, `Verify immutable v1 fixture and upcast`. So attempt 1 is a
timeout cancellation and not a test result, which is the rule this page opens
with.

**The 26:27 is not this task's cost, and that is measured rather than argued.**
Splitting the step by the job log's own timestamps:

| Part | Time |
|---|---:|
| whole-workspace compilation, to `Finished \`test\` profile` | 2:03 |
| running 143 test binaries | 24:18 |
| — of which `academic_store`'s `unittests src/lib.rs` | 8:14 |
| — of which `academic-transcript`'s `tests/projection_format.rs` | 2:51 |
| — of which **all five `academic-capture` binaries together** | **1.84 s** |

`academic-capture` is 0.13% of the step it is accused of blowing. The dominant
row is a pre-existing crate's unit tests.

The same-commit rerun read **18:41**, and the rebased head read **16:35**, both
inside the range this page already holds for that job — 12:16, 13:47, 14:08,
14:55, 15:28, 16:35, 17:09, 18:03, 18:41, 20:18. Two readings of *one* commit,
30:14 and 18:41, is an 11:33 spread on identical work, which is larger than any
spread this page had recorded before and is the reason attempt 1 is kept.

**The 80% review trigger has now been reached once and is not discharged by the
rerun.** What the rule asks for is a split, an admitted cache, or a measured
timeout change; none is made here, because none of them is this task's to make
and the cost that crossed the line is not this task's either. The trigger is
recorded so the next person meets it as a number.

### `rust-features-windows-latest` hit the launch error again

Attempt 1 failed at `cpu_memory_time_output_limits_are_enforced` with

```text
Error: Launch { path: "D:\a\Academic-Platform\Academic-Platform\target\debug\academic-worker-probe.exe",
        detail: "CreateProcessW returned 0 (last error 2)" }
```

7 passed, 1 failed, and the four steps after it were skipped. This is the fourth
occurrence of the signature the section below names, and the same-commit rerun
passed, taking the job to green. The commit under it adds no line to
`academic-worker`, and the identical step list passed on
`rust-features-windows-11-arm` in the same attempt.

## The `P2-X1` run

`P2-X1` added one workspace member, `academic-desktop`, one pnpm workspace
package, `@academic-os/ui`, and two scans to `tools/phase1-scaffold-policy.test.mjs`.
Workspace membership and a default-workspace test are both triggers the refresh
rule names. The job count is unchanged at **17**: the new crate compiles inside
the `cargo test --workspace --locked` step the five `rust-default-*` jobs
already run, and everything JavaScript runs inside `pnpm-contracts`. No new job,
no new step, no system package, no frontend bundler.

Run
[33720001800](https://github.com/dnhynk/Academic-Platform/actions/runs/33720001800)
completed **17/17** on `b35ae58`.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:05 | 5:00 | 1.7% |
| `rust-default-ubuntu-latest` | 5:50 | 30:00 | 19.4% |
| `rust-default-ubuntu-24.04-arm` | 4:49 | 30:00 | 16.1% |
| `rust-default-windows-latest` | 17:09 | 30:00 | 57.2% |
| `rust-default-windows-11-arm` | 16:06 | 30:00 | 53.7% |
| `rust-default-macos-latest` | 4:43 | 30:00 | 15.7% |
| `rust-features-ubuntu-latest` | 3:44 | 30:00 | 12.4% |
| `rust-features-ubuntu-24.04-arm` | 2:51 | 30:00 | 9.5% |
| `rust-features-windows-latest` | 6:36 | 30:00 | 22.0% |
| `rust-features-windows-11-arm` | 5:54 | 30:00 | 19.7% |
| `rust-features-macos-latest` | 3:49 | 30:00 | 12.7% |
| `phase1-exit-ubuntu-latest` | 4:06 | 45:00 | 9.1% |
| `phase1-exit-windows-latest` | 10:31 | 45:00 | 23.4% |
| `encrypted-store-lane-ubuntu-latest` | 3:22 | 45:00 | 7.5% |
| `encrypted-portability-lane-ubuntu-latest` | 5:02 | 45:00 | 11.2% |
| `rotation-orchestration-lane-ubuntu-latest` | 7:06 | 45:00 | 15.8% |
| `pnpm-contracts` | 0:57 | 15:00 | 6.3% |

**`pnpm-contracts` moved +0:10, from 0:47 to 0:57.** That job is where every
line of this task's JavaScript runs — a new package's lint, typecheck, build and
17 tests, and two new scans inside the root `pnpm test` — so it is the one
number that isolates this change's cost. Ten seconds, at 6.3% of a 15-minute
limit. A measurement of the same tree on the run before the rebase read 0:57 as
well, so the figure is stable across two runs rather than one.

**`rust-default-windows-latest` is the slowest job at 57.2%**, up 2:14 from the
`P2-M2` reading of 14:55. That is not read here as a new cost, and the reasons
are checkable rather than convenient. This page has now recorded that one job at
20:18, 14:08, 12:16, 15:28, 13:47, 14:55, 18:03 and 17:09 — the new reading sits
inside that range and below its maximum, and the two readings of *this same
tree*, on the pre-rebase and post-rebase runs, were 18:03 and 17:09, which is a
0:54 spread on identical work. The default lane gained one workspace member
whose whole test tree is nine files finishing in well under a second locally on
both hosts.

`rust-default-windows-11-arm` is the second slowest at 53.7%, up 3:37, and moved
the *opposite* way between this task's own two runs — 13:11 then 16:06 — which
is the same spread seen from the other side.

Every other movement against `P2-M2` is about a minute or less in either
direction, four of them downward: `rust-default-macos-latest` −2:09,
`rust-features-ubuntu-24.04-arm` −0:07, `dependency-source-preflight` −0:04, and
`rust-default-ubuntu-latest` +0:38, `phase1-exit-windows-latest` +1:54.

**No job reaches the 80% review trigger.** For the 30-minute Rust jobs that line
is 24:00, and the worst reading this page holds for any label is 20:18. Size
headroom off that maximum — 67.7% — rather than off any single run.

## A Windows failure that is not a test result

This page exists because a timeout cancellation is not a test failure. There is
a second thing on this repository's Windows jobs that is not one either, and it
is not a timeout.

`rust-features-windows-latest` failed once at
`resource_receipt_is_recorded_per_run` in the worker sandbox lane, with

```text
Error: Launch { path: "…\target\debug\academic-worker-probe.exe",
        detail: "CreateProcessW returned 0 (last error 2)" }
```

`last error 2` is `ERROR_FILE_NOT_FOUND` for a probe binary the same job had
just linked; seven of that suite's eight rows passed around it. Re-running that
job on the same commit passed 8/8. The same failure appeared once on a Windows
developer machine in the same lane, on a tree that touched no part of
`academic-worker`, and re-running that exact command on the unchanged tree
passed there too.

`P2-X1` hit it a third time, at `malicious_plugin_corpus_is_contained`, and that
occurrence is the cleanest falsification this page has: run
[33716669384](https://github.com/dnhynk/Academic-Platform/actions/runs/33716669384)
was a commit of **three Markdown files and nothing else**, on a parent whose own
run was 17/17, and the rerun of that one job took it to 17/17. A signature that
appears on a commit no compiler reads is not a property of the change. That
branch was later rebased, so the commit that run names is no longer reachable;
the observation is kept because the rule it exercises is, and the run is.

So the rule is the same as the timeout rule: a Windows job that fails inside
`Test the worker sandbox lane` with a `CreateProcessW` launch error is
falsified or confirmed by re-running that job on the same commit, and only a
failure that survives the rerun is a test result. It is recorded here rather
than in the worker sandbox contract because what is unreliable is the hosted
Windows filesystem right after a link, not anything the sandbox claims.

Seen on runs
[33697656939](https://github.com/dnhynk/Academic-Platform/actions/runs/33697656939)
(failed, then 17/17 on rerun of the one job), once locally on Windows in the
`P2-G6` verification, and again on
[33715585336](https://github.com/dnhynk/Academic-Platform/actions/runs/33715585336)
in `P2-M2`.

That third sighting is the sharpest one this page has, because the two halves of
the rule were both executed against a tree that touches no part of
`academic-worker`. `resource_receipt_is_recorded_per_run`,
`cpu_memory_time_output_limits_are_enforced` and
`malicious_plugin_corpus_is_contained` failed together with the same
`CreateProcessW` launch error while the other five rows passed; the same suite
passed **8/8 on a Windows developer machine on that exact commit**, and
re-running only that job on that exact commit passed too. The commit under it
changed one test file in `crates/proposal` and one contract page, and the
identical job had already passed on the parent commit.

### A second signature, in a different job

`P2-M1` hit one that is the same kind and not the same failure, so it is
recorded separately rather than filed under the launch error above.
`phase1-exit-windows-latest` failed at `phase1_exit_rejects_real_data` with

```text
Error: Io { kind: BrokenPipe }
```

That test starts a real daemon and speaks the local IPC protocol to it, so a
broken pipe is the client losing its peer rather than an assertion about what
the daemon decided; the six rows around it passed. What makes the reading
checkable rather than convenient is the commit it happened on: run
[33705304808](https://github.com/dnhynk/Academic-Platform/actions/runs/33705304808)
at `6748cd6`, whose entire delta from the 17/17 run
[33704083224](https://github.com/dnhynk/Academic-Platform/actions/runs/33704083224)
is 46 lines of Markdown in this file, which no test reads. The same job on the
parent commit passed, the suite passes locally on Windows on that tree — once
whole and three times isolated — and the same-commit rerun of that one job
passed, taking the run to 17/17. Attempt 1's failure is kept: query
`actions/runs/33705304808/attempts/1/jobs` rather than the run's current jobs,
which is what the refresh rule above says to do for exactly this reason.

So the rule extends to a second shape: a Windows job that fails with an I/O
error against a process it started -- a launch error before the process runs, or
a broken pipe after it does -- is falsified or confirmed by a same-commit rerun,
and only a failure that survives one is a test result. Two signatures are not a
licence to rerun anything: an assertion failure is a test result on the first
observation, and neither of these is an assertion.

## The `P2-R1` run

`P2-R1` adds one workspace member, `academic-repository`, and a canonical-store
migration, `0012`, both of which the refresh rule names as triggers. Run
[33729501242](https://github.com/dnhynk/Academic-Platform/actions/runs/33729501242)
at `4509864` completed 17/17.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:04 | 5:00 | 1.3% |
| `rust-default-ubuntu-latest` | 5:40 | 30:00 | 18.9% |
| `rust-default-ubuntu-24.04-arm` | 4:52 | 30:00 | 16.2% |
| `rust-default-windows-latest` | 16:56 | 30:00 | 56.4% |
| `rust-default-windows-11-arm` | 14:26 | 30:00 | 48.1% |
| `rust-default-macos-latest` | 7:16 | 30:00 | 24.2% |
| `rust-features-ubuntu-latest` | 3:43 | 30:00 | 12.4% |
| `rust-features-ubuntu-24.04-arm` | 2:52 | 30:00 | 9.6% |
| `rust-features-windows-latest` | 5:58 | 30:00 | 19.9% |
| `rust-features-windows-11-arm` | 5:30 | 30:00 | 18.3% |
| `rust-features-macos-latest` | 2:58 | 30:00 | 9.9% |
| `phase1-exit-ubuntu-latest` | 4:19 | 45:00 | 9.6% |
| `phase1-exit-windows-latest` | 9:52 | 45:00 | 21.9% |
| `encrypted-store-lane-ubuntu-latest` | 3:21 | 45:00 | 7.4% |
| `encrypted-portability-lane-ubuntu-latest` | 5:11 | 45:00 | 11.5% |
| `rotation-orchestration-lane-ubuntu-latest` | 7:06 | 45:00 | 15.8% |
| `pnpm-contracts` | 0:58 | 15:00 | 6.4% |

The slowest job is `rust-default-windows-latest` at 56.4%. That is a **fourth**
reading of the job the `P2-RF11` section says has no single-reading budget:
20:18, 14:08, 12:16 and now 16:56 on one workflow — 67.7%, 47.1%, 40.9% and
56.4%. The new reading sits inside the range those three established rather than
extending it, so the guidance is unchanged: size headroom off 67.7% and treat
each further reading as evidence about the range. `rust-default-windows-11-arm`
is beside it at 48.1%, its own third reading in a 42.7%–48.1% band. Every Linux,
Linux ARM and macOS Rust job is at or below 24.2%.

The new member adds one crate to `cargo clippy --workspace --all-targets` and
two test binaries plus five in-crate store tests to `cargo test --workspace`.
`encrypted-store-lane-ubuntu-latest` is the job migration `0012` changes, and it
is at 7.4% — unchanged from the `P2-G6` reading of the same job at the same
percentage, which is what one more migration in the creation transaction costs
on that lane.
