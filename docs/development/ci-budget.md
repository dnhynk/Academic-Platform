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

## Latest run

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

So the rule is the same as the timeout rule: a Windows job that fails inside
`Test the worker sandbox lane` with a `CreateProcessW` launch error is
falsified or confirmed by re-running that job on the same commit, and only a
failure that survives the rerun is a test result. It is recorded here rather
than in the worker sandbox contract because what is unreliable is the hosted
Windows filesystem right after a link, not anything the sandbox claims.

Seen on runs
[33697656939](https://github.com/dnhynk/Academic-Platform/actions/runs/33697656939)
(failed, then 17/17 on rerun of the one job) and once locally on Windows in the
`P2-G6` verification.
