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

The first hosted run of this split must replace this paragraph with a 17-row
table using the refresh method above. Until that run completes, the only
measured projection is the pre-split subtraction: on the timeout attempt the
default group accounted for 15:48 before cold-job effects, and in run
33668524442 it accounted for 15:06. The feature group's cold-start duration is
not claimed before hosted measurement.
