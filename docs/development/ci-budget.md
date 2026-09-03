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

A refreshed table is one reading of a distribution, not the distribution. Where
this page states headroom for a label it states the **range** — how many
readings, the smallest, the median and the largest — because the readings that
decide a timeout are the ones no single run shows. Enumerate the workflow's own
history rather than this page's tables to rebuild one:

```text
gh api repos/dnhynk/Academic-Platform/actions/workflows/ci.yml/runs?per_page=100
gh api repos/dnhynk/Academic-Platform/actions/runs/<run-id>/attempts/<n>/jobs?per_page=100
```

and keep every attempt, because the attempts that were cancelled are the ones
worth reading. Separate the two kinds of cancellation before counting: the
workflow's `cancel-in-progress` concurrency group cancels a superseded push at
whatever time it had reached, which is not a reading of anything, while a
cancellation at the limit is a right-censored reading — the job's real cost is
at least that, and how much more is unknown.

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

The workflow now materializes 22 required jobs. The five `rust-default-*` jobs
retain formatting, default clippy, the workspace test apart from
`academic-store` (including doc tests), and all fixture commands. The five
`rust-store-*` jobs run `academic-store`'s default-feature tests on the same
five labels — the second half of that workspace test, split off for the reason
[the store split](#the-store-split) records. The five `rust-features-*` jobs
retain every encrypted-object, rotation/retention, transcript, native-worker and
capture clippy/test command on the same five labels. All three groups have a
30-minute limit.

The first run on the split workflow,
[33675718049](https://github.com/dnhynk/Academic-Platform/actions/runs/33675718049),
completed at `25e6221`. It is the reading the paragraph below is about, and it
is the highest this page holds for `rust-default-windows-latest` outside the
cancellation recorded further down.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:10 | 5:00 | 3.3% |
| `rust-default-ubuntu-latest` | 4:54 | 30:00 | 16.3% |
| `rust-default-ubuntu-24.04-arm` | 3:48 | 30:00 | 12.7% |
| `rust-default-windows-latest` | 20:18 | 30:00 | 67.7% |
| `rust-default-windows-11-arm` | 12:54 | 30:00 | 43.0% |
| `rust-default-macos-latest` | 4:19 | 30:00 | 14.4% |
| `rust-features-ubuntu-latest` | 2:23 | 30:00 | 7.9% |
| `rust-features-ubuntu-24.04-arm` | 2:24 | 30:00 | 8.0% |
| `rust-features-windows-latest` | 5:26 | 30:00 | 18.1% |
| `rust-features-windows-11-arm` | 4:26 | 30:00 | 14.8% |
| `rust-features-macos-latest` | 2:51 | 30:00 | 9.5% |
| `phase1-exit-ubuntu-latest` | 2:50 | 45:00 | 6.3% |
| `phase1-exit-windows-latest` | 7:43 | 45:00 | 17.1% |
| `encrypted-store-lane-ubuntu-latest` | 3:08 | 45:00 | 7.0% |
| `encrypted-portability-lane-ubuntu-latest` | 4:03 | 45:00 | 9.0% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:59 | 45:00 | 15.5% |
| `pnpm-contracts` | 0:50 | 15:00 | 5.6% |

The slowest job is `rust-default-windows-latest` at 67.7%. Its 20:18
also shows that splitting alone while retaining the old 20-minute timeout
would still have failed on this runner. The independently completed feature
job used 18.1% instead of extending that default job by another 5:26. Windows
ARM is the next-highest Rust default at 43.0%; every Linux, Linux ARM, and macOS
Rust job is at or below 16.3%. The table has no `rust-store-*` row because that
group did not exist on this run.

## Latest run

`P2-L3` adds one workspace member, `academic-transcription`, and a
default-workspace test: two acceptance binaries and a `trybuild` harness over
seven programs, all compiled and run by the
`cargo test --workspace --exclude academic-store --locked` step the five
`rust-default-*` jobs already have. Workspace membership and a default-workspace
test are two of the four triggers the refresh rule names. It adds no pnpm
package, no feature lane, no migration, no system package and no change to
`.github/workflows/ci.yml`, so **the job count is unchanged at 22**.

**Four runs, each 22/22 on the first attempt, no rerun.** The branch was rebased
twice while `main` moved — onto `P2-U1`, then onto `P2-R2` — with `git rebase`
and never `git merge`, so only the last measures a tree that still exists. The
earlier three are kept, because the comparison across them is the finding.

| Run | Head | Tree |
|---|---|---|
| [33776759904](https://github.com/dnhynk/Academic-Platform/actions/runs/33776759904) | `cb83c60` | the crate as first pushed |
| [33780711210](https://github.com/dnhynk/Academic-Platform/actions/runs/33780711210) | `5a0ba5c` | plus one `fix(transcription)` commit |
| [33784209481](https://github.com/dnhynk/Academic-Platform/actions/runs/33784209481) | `9d289ab` | rebased onto `P2-U1` |
| [33787226201](https://github.com/dnhynk/Academic-Platform/actions/runs/33787226201) | `7795741` | **rebased onto `P2-R2`; the last tree CI compiles differently** |

A run whose head is a Markdown-only commit is not tabulated, because it compiles
the same tree as the tabulated run below it. Two such runs were observed at
22/22 —
[33778983581](https://github.com/dnhynk/Academic-Platform/actions/runs/33778983581)
on `be13b28` and
[33788795012](https://github.com/dnhynk/Academic-Platform/actions/runs/33788795012)
on `e5a2fb0` — and the run triggered by the commit that last edited this section
is a third of that kind, whose result is by construction not in this file. No run
on this branch was cancelled by the concurrency group.

| Required job | 33776759904 | 33780711210 | 33784209481 | **33787226201** | Limit | Worst |
|---|---:|---:|---:|---:|---:|---:|
| `dependency-source-preflight` | 0:08 | 0:06 | 0:08 | **0:08** | 5:00 | 2.7% |
| `rust-default-ubuntu-latest` | 5:05 | 5:14 | 4:39 | **5:03** | 30:00 | 17.4% |
| `rust-default-ubuntu-24.04-arm` | 4:35 | 4:36 | 4:29 | **4:45** | 30:00 | 15.8% |
| `rust-default-windows-latest` | 14:12 | 11:10 | 14:58 | **12:13** | 30:00 | 49.9% |
| `rust-default-windows-11-arm` | 11:50 | 10:48 | 11:32 | **11:49** | 30:00 | 39.4% |
| `rust-default-macos-latest` | 7:26 | 6:44 | 7:51 | **7:09** | 30:00 | 26.2% |
| `rust-store-ubuntu-latest` | 1:30 | 1:26 | 2:54 | **1:23** | 30:00 | 9.7% |
| `rust-store-ubuntu-24.04-arm` | 1:10 | 1:30 | 1:36 | **1:13** | 30:00 | 5.3% |
| `rust-store-windows-latest` | 5:29 | 9:15 | 5:12 | **6:46** | 30:00 | 30.8% |
| `rust-store-windows-11-arm` | 5:53 | 6:32 | 6:12 | **5:29** | 30:00 | 21.8% |
| `rust-store-macos-latest` | 1:28 | 1:42 | 2:07 | **2:02** | 30:00 | 7.1% |
| `rust-features-ubuntu-latest` | 4:05 | 3:53 | 4:02 | **3:45** | 30:00 | 13.6% |
| `rust-features-ubuntu-24.04-arm` | 3:13 | 2:54 | 3:00 | **3:25** | 30:00 | 11.4% |
| `rust-features-windows-latest` | 6:18 | 6:10 | 6:36 | **6:15** | 30:00 | 22.0% |
| `rust-features-windows-11-arm` | 6:13 | 5:35 | 6:12 | **6:21** | 30:00 | 21.2% |
| `rust-features-macos-latest` | 3:29 | 2:21 | 3:54 | **4:04** | 30:00 | 13.6% |
| `phase1-exit-ubuntu-latest` | 4:19 | 5:47 | 4:24 | **4:11** | 45:00 | 12.9% |
| `phase1-exit-windows-latest` | 8:30 | 9:43 | 17:18 | **12:26** | 45:00 | 38.4% |
| `encrypted-store-lane-ubuntu-latest` | 3:23 | 2:40 | 3:08 | **3:18** | 45:00 | 7.5% |
| `encrypted-portability-lane-ubuntu-latest` | 5:07 | 3:59 | 4:57 | **5:20** | 45:00 | 11.9% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:50 | 6:42 | 7:16 | **6:50** | 45:00 | 16.1% |
| `pnpm-contracts` | 1:26 | 0:51 | 1:05 | **0:56** | 15:00 | 9.6% |

### What four readings say that one did not

Written from the first run alone, this section read
`rust-default-windows-latest` at 14:12 as the new member's cost landing in the
remainder job exactly where `--workspace --exclude academic-store` routes it.
**That reading is withdrawn.** It is replaced rather than left standing beside a
correction, and what replaced it is a method rather than a number.

The split gave this page **controls it did not set out to create**: jobs whose
input this task does not touch, running beside the one that grew.

* `rust-store-windows-latest` runs `-p academic-store`, a crate outside this
  diff. It reads **5:29, 9:15, 5:12, 6:46** — a 4:03 spread, and its *slowest*
  reading is on the tree with the least store code in it.
* `phase1-exit-windows-latest` is untouched by this task in every run. It reads
  **8:30, 9:43, 17:18, 12:26** — an 8:48 spread, and it doubled once.

Against controls moving by 4:03 and 8:48, `rust-default-windows-latest`'s
11:10–14:58 says nothing about what one workspace member costs. **None of the
four runs isolates it, and this page no longer claims one does.**

The spans, as spans: `rust-default-windows-latest` **11:10–14:58** over seven
post-split readings (11:20, 12:43, 12:25, 14:12, 11:10, 14:58, 12:13);
`rust-store-windows-latest` **5:12–9:15** over seven (5:33, 5:13, 5:43, 5:29,
9:15, 5:12, 6:46). Sizing is unchanged and is taken from none of them: headroom
comes off the pre-split worst case of **67.7%** and the 80% review trigger for a
30:00 job is **24:00**. The worst job across all four runs is 49.9%, which is
30.1 points under it.

**The method this leaves behind.** A task measuring its own cost on a Windows
label should read a job the split froze — `rust-store-*` or `phase1-exit-*` —
beside `rust-default-*` in the same run, and treat any difference smaller than
the control's own movement as unmeasured.

### What the new member adds, stated without a timing claim

One crate joins `cargo clippy --workspace --all-targets --locked`; three test
binaries — 16 acceptance rows, 12 source-shape rows, and one `trybuild` harness
over seven programs — join the workspace test in the same job. The harness is the
expensive one of the three, because it compiles seven programs and compares seven
committed diagnostics. **What that costs in seconds is not measured here**, for
the reason above.

`encrypted-store-lane-ubuntu-latest` reads 3:23, 2:40, 3:08 and 3:18, against
3:25 on the split's run: this task adds no migration, so `STORE_MIGRATION_SQL`
and the admission fingerprint that lane asserts against are unchanged by it.
`P2-U1`'s `0014` is in the last two runs' trees and the lane is unmoved by that
too.

**No `CreateProcessW` launch failure occurred on any of the four runs.**
`rust-features-windows-latest` was green on the first attempt of each. Four
attempts against a measured 14.3% rate falsifies nothing; it is noted so the next
reader does not count them as evidence about that signature. This task **did**
hit that signature locally, which is the subsection under
[a Windows failure that is not a test result](#a-windows-failure-that-is-not-a-test-result).

## The store split

### What changed

`rust-default-*` runs `cargo test --workspace --exclude academic-store --locked`
and a new `rust-store-*` group runs `cargo test -p academic-store --locked` on
the same five labels. The job count goes 17 → 22.

No command and no label was dropped. `academic-store` is still linted by the
unchanged `cargo clippy --workspace --all-targets --locked` in `rust-default-*`,
still tested under `sqlcipher-store` by `encrypted-store-lane-ubuntu-latest`,
and the README's local block still runs the whole workspace in one `cargo test`.
`--workspace --exclude` also means a member added later lands in the remainder
job with no edit to either command.

`tools/verify-contracts.mjs` reads the workflow and fails if a package excluded
from the workspace test is run by no job, is run only under a non-default
feature set, or is run on fewer than the five hosted labels; five mutations of
the workflow are asserted to fail it.

### What it was sized against

Every completed reading of every post-split Rust label, enumerated from the
workflow's own run history rather than from this page's tables — 57 runs, every
attempt of each:

| Label | n | min | median | p90 | max | max/median |
|---|---:|---:|---:|---:|---:|---:|
| `rust-default-ubuntu-latest` | 57 | 3:28 | 5:10 | 5:46 | 5:53 | 1.14 |
| `rust-default-ubuntu-24.04-arm` | 57 | 3:42 | 4:33 | 4:57 | 5:14 | 1.15 |
| `rust-default-windows-latest` | 53 | 12:16 | 14:55 | 17:09 | 20:18 | 1.36 |
| `rust-default-windows-11-arm` | 54 | 10:46 | 13:31 | 15:26 | 16:15 | 1.20 |
| `rust-default-macos-latest` | 57 | 3:53 | 5:50 | 7:25 | 10:37 | 1.82 |
| `rust-features-ubuntu-latest` | 59 | 1:49 | 3:23 | 3:50 | 4:05 | 1.21 |
| `rust-features-ubuntu-24.04-arm` | 60 | 2:12 | 2:50 | 3:02 | 3:14 | 1.14 |
| `rust-features-windows-latest` | 48 | 4:08 | 5:52 | 6:27 | 7:02 | 1.20 |
| `rust-features-windows-11-arm` | 57 | 4:12 | 5:29 | 6:06 | 6:35 | 1.20 |
| `rust-features-macos-latest` | 60 | 1:54 | 2:59 | 3:41 | 3:51 | 1.29 |

**Not one of those 53 readings reached 24:00, the 80% line.** The trigger was
reached by a 54th observation that is not in the table because it did not
complete: the 30:14 cancellation recorded below, which is 2.03× that label's
median and 1.49× the largest reading beside it. Two readings of *one* commit,
30:14 and 18:41, are 1.62× apart.

So the 80% row was not the tail of the spread the other nine labels show. Those
nine sit at 1.14×–1.82× of their own medians, and the 30:14 sits outside every
one of them. **A single reading is not a budget for this label and neither is a
single percentile: what the table above is for is the range.**

The other seven `rust-default-windows-latest` records that are not in the table
were cancelled by the workflow's own `cancel-in-progress` concurrency group when
a newer push superseded them, at 1:35 to 12:39. A concurrency cancellation is
not a timeout and not a reading.

### Where the time goes, and what that ruled out

`Test Rust workspace` on the cancelled attempt was 26:27: **2:03 compiling and
24:18 running 143 test binaries.** By package, on that attempt against the same
step on the rebased head that read 12:39:

| Package | cancelled attempt | rebased head | share |
|---|---:|---:|---:|
| `academic-store` | 11:05 | 3:29 | 45.6% / 36.7% |
| `academic-core` | 5:13 | 1:18 | 21.5% / 13.7% |
| `academic-portability` | 2:25 | 1:03 | 9.9% / 11.1% |
| `academic-vault` | 2:11 | 0:24 | 8.9% / 4.3% |
| every other package together | 3:25 | 3:15 | 14.1% / 34.2% |

The single largest row is `academic_store`'s `unittests src/lib.rs`: 52 tests in
494 s on the cancelled attempt, 57 tests in 132 s on the rebased head, and 56
tests in **3.4 s on `ubuntu-latest` in the same run** — 2.31 s per test on a
hosted Windows runner against 0.06 s on Linux. Those tests open real databases
under the connection policy `crates/store/src/connection.rs` pins,
`PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL`, in the runner's
temporary directory.

**What was slow on the cancelled attempt was not compilation**, and three
observations of essentially one tree separate the two:

| | cancelled attempt | same commit, rerun | rebased head |
|---|---:|---:|---:|
| job elapsed | 30:14 | 18:41 | 16:35 |
| `cargo clippy --workspace --all-targets` step | **0:51** | 0:53 | 1:07 |
| compile inside the test step | **2:03** | 2:15 | 3:02 |
| test binaries: 143, 143, 145 | **24:18** | 12:02 | 9:31 |

Both compilation steps were the *fastest* of the three on the attempt that was
cancelled; only file-backed test execution scaled, by 2.02× against the same
commit. Whatever the runner-side cause is, it is not one a faster build would
have answered. That is what decided between the options below.

### The options

**A cache was rejected on a number rather than a principle.** Compilation on the
cancelled attempt was 2:51 of 30:14 — 9.4% — and it was already the fastest
compilation of the three. A `target/` cache that removed *all* of it would have
landed that attempt at about 27:20: inside the limit, by 2:40, on a label whose
measured spread is 1.36× of a 14:55 median. That is less margin than one draw of
the spread this page has measured, bought on the part that did not scale, and it
is not free — restoring and saving a Windows `target/` directory is minutes of
its own charged to every job. The Cargo *registry* is already cached by
`actions/cache`; a new caching action would additionally need the
`CONTRIBUTING.md` admission procedure and a `docs/security/` receipt.

**Raising the limit was rejected as the load-bearing change.** To absorb the
observed 2.03× draw at the label's current median of 16:35 the limit would have
to be about 34:00, and at 45:00 it would bind again once the median reaches
22:12. That median has moved 14:24 → 16:35 across the 7 workspace members added
during these 53 readings, which is **+19 s per member**, so 45 minutes buys
about 18 more members and then asks the same question with a larger denominator
— and with every percentage on this page reset so a growing base reads as
headroom. The limit stays at 30:00.

**Removing tests was not considered.** The README verification block is a
contract; buying time by shrinking it is not available.

**The split acts on the row the measurement names.** It moves 37%–46% of the
lane's test execution into a second runner draw, so a slow disk is charged to
half the work rather than all of it, and it lowers the median critical path of
the default lane on every ordinary run rather than only on the tail.

### When this is due again

The split does nothing about growth: +19 s per member still accrues, now divided
between two jobs. The number to watch is the same 80% line, 24:00, and the
measured run below is the first reading of where the two halves start from. What
the split changed is which reading is at risk — this page's rule is unchanged
and is now easier to apply, because a group whose worst reading crosses 24:00
names the package to move next, and the guard in `verify-contracts.mjs` makes
moving one a two-line workflow edit that cannot silently drop it.

## The store-split run

This task changes `.github/workflows/ci.yml`, which the refresh rule names as a
trigger, and changes nothing else a test reads: no workspace member, no test, no
feature lane. So the readings below are the same tree as the run before them,
and the difference between the two is the split rather than growth.

Run
[33746159023](https://github.com/dnhynk/Academic-Platform/actions/runs/33746159023)
completed **22/22** on `2503fe8`, first attempt, no rerun.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:04 | 5:00 | 1.3% |
| `rust-default-ubuntu-latest` | 4:51 | 30:00 | 16.2% |
| `rust-default-ubuntu-24.04-arm` | 4:06 | 30:00 | 13.7% |
| `rust-default-windows-latest` | 11:20 | 30:00 | 37.8% |
| `rust-default-windows-11-arm` | 10:01 | 30:00 | 33.4% |
| `rust-default-macos-latest` | 5:37 | 30:00 | 18.7% |
| `rust-store-ubuntu-latest` | 1:38 | 30:00 | 5.4% |
| `rust-store-ubuntu-24.04-arm` | 1:15 | 30:00 | 4.2% |
| `rust-store-windows-latest` | 5:33 | 30:00 | 18.5% |
| `rust-store-windows-11-arm` | 6:32 | 30:00 | 21.8% |
| `rust-store-macos-latest` | 1:34 | 30:00 | 5.2% |
| `rust-features-ubuntu-latest` | 3:57 | 30:00 | 13.2% |
| `rust-features-ubuntu-24.04-arm` | 3:03 | 30:00 | 10.2% |
| `rust-features-windows-latest` | 5:00 | 30:00 | 16.7% |
| `rust-features-windows-11-arm` | 5:35 | 30:00 | 18.6% |
| `rust-features-macos-latest` | 3:22 | 30:00 | 11.2% |
| `phase1-exit-ubuntu-latest` | 4:08 | 45:00 | 9.2% |
| `phase1-exit-windows-latest` | 9:49 | 45:00 | 21.8% |
| `encrypted-store-lane-ubuntu-latest` | 3:25 | 45:00 | 7.6% |
| `encrypted-portability-lane-ubuntu-latest` | 5:08 | 45:00 | 11.4% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:51 | 45:00 | 15.2% |
| `pnpm-contracts` | 0:57 | 15:00 | 6.3% |

**`rust-default-windows-latest` reads 11:20, below the *smallest* of the 53
readings tabulated above** — 12:16 — and 3:35 under their median. Its other half,
`rust-store-windows-latest`, reads 5:33. The two together are 16:53 of runner
time against a 14:55 median for the one job they replace: about two minutes more
spent in total, to take 3:35 off the lane's critical path.

**The store group's worst label is `windows-11-arm` at 6:32, not
`windows-latest`**, and `rust-default-windows-11-arm` fell further than
`windows-latest` did — 13:31 median to 10:01. The worst job anywhere on this run
is 37.8%.

**One run is one reading, which is the thing this page keeps saying.** It does
not retire the 12:16–20:18 span plus one cancellation that the section above
holds for the pre-split job; what it establishes is where the two halves start
from. Add the next readings of `rust-default-*` and `rust-store-*` here as a
range rather than as a replacement, and re-read the 80% line — 24:00 — against
the largest of them.

### Three readings of the same tree

Two docs-only follow-ups,
[33748404269](https://github.com/dnhynk/Academic-Platform/actions/runs/33748404269)
on `2bf7cf4` and
[33750554768](https://github.com/dnhynk/Academic-Platform/actions/runs/33750554768)
on `8c48867`, also completed **22/22** on the first attempt. Each differs from
the run before it by Markdown no test reads, so all three are readings of one
tree, and the page's own rule says to keep them as a range rather than let the
last overwrite the others:

| Label | 33746159023 | 33748404269 | 33750554768 |
|---|---:|---:|---:|
| `rust-default-windows-latest` | 11:20 | 12:43 | 12:25 |
| `rust-store-windows-latest` | 5:33 | 5:13 | 5:43 |
| `rust-default-windows-11-arm` | 10:01 | 11:06 | 10:59 |
| `rust-store-windows-11-arm` | 6:32 | 5:06 | 5:40 |

So the split's readings so far are **`rust-default-windows-latest` 11:20–12:43
and `rust-store-windows-latest` 5:13–5:43**, and the worst job on any of the
three is 42.4%. All three default readings sit below the 12:16 minimum of the 53
pre-split readings, and the 80% line for these jobs remains 24:00. Three
readings are a start, not a distribution: the section above needed 53 before it
could say anything about a tail.

`rust-features-windows-latest` was green on the first attempt of all three runs.
That is three attempts against a measured 14.3% failure rate, so it falsifies nothing;
it is noted only so the next reader does not count them as clean runs for that
signature.

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

## The `P2-L2` run

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
| — of which `academic-core`'s `tests/projection_format.rs` | 2:51 |
| — of which **all five `academic-capture` binaries together** | **1.84 s** |

`academic-capture` is 0.13% of the step it is accused of blowing. The dominant
row is a pre-existing crate's unit tests.

The same-commit rerun read **18:41**, and the rebased head read **16:35**, both
inside the range this page already holds for that job — 12:16, 13:47, 14:08,
14:55, 15:28, 16:35, 16:56, 17:09, 18:03, 18:41, 20:18. Two readings of *one* commit,
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

## The `T161` run

`T161` adds no workspace member, no pnpm package, no feature lane, no migration
and no system package. What it adds is **three named acceptance rows and two
source scans inside two crates that the `cargo test --workspace --locked` step
already compiles**, which is the refresh rule's default-workspace-test trigger
and nothing else. The job count is unchanged at **17**.

Run
[33750252558](https://github.com/dnhynk/Academic-Platform/actions/runs/33750252558)
completed **17/17 on `3ff5681`**, first attempt, no rerun. That head is the
branch as it stood before this section was written; the commit carrying this
table is the only thing on top of it, which is the same arrangement
[the `P2-U6` run](#the-p2-u6-run) below records.

The branch was rebased once, onto `P2-U6` after it merged, with `git rebase`.
The readings its pre-rebase heads took are not carried here: two of those runs
were **cancelled by the concurrency group** when the next push superseded them,
which is not a test result and is not evidence either way.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:04 | 5:00 | 1.3% |
| `rust-default-ubuntu-latest` | 6:01 | 30:00 | 20.1% |
| `rust-default-ubuntu-24.04-arm` | 4:49 | 30:00 | 16.1% |
| `rust-default-windows-latest` | 16:19 | 30:00 | 54.4% |
| `rust-default-windows-11-arm` | 16:18 | 30:00 | 54.3% |
| `rust-default-macos-latest` | 5:03 | 30:00 | 16.8% |
| `rust-features-ubuntu-latest` | 3:11 | 30:00 | 10.6% |
| `rust-features-ubuntu-24.04-arm` | 2:57 | 30:00 | 9.8% |
| `rust-features-windows-latest` | 5:19 | 30:00 | 17.7% |
| `rust-features-windows-11-arm` | 6:00 | 30:00 | 20.0% |
| `rust-features-macos-latest` | 4:04 | 30:00 | 13.6% |
| `phase1-exit-ubuntu-latest` | 4:19 | 45:00 | 9.6% |
| `phase1-exit-windows-latest` | 10:18 | 45:00 | 22.9% |
| `encrypted-store-lane-ubuntu-latest` | 3:44 | 45:00 | 8.3% |
| `encrypted-portability-lane-ubuntu-latest` | 5:00 | 45:00 | 11.1% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:45 | 45:00 | 15.0% |
| `pnpm-contracts` | 0:47 | 15:00 | 5.2% |

**`rust-default-windows-latest` reads 16:19, or 54.4%** — down from `P2-U6`'s
18:12 on a tree that is larger, and the fourth lowest of the thirteen readings
this page now holds for that job: 12:16, 13:47, 14:08, 14:55, 15:28, 16:19,
16:35, 16:56, 17:09, 18:03, 18:12, 18:41 and 20:18. It sits inside the band
rather than extending it, so the guidance the `P2-L2` section gives stands
unchanged: size headroom off 20:18.

**`rust-default-windows-11-arm` at 16:18 is the highest reading this page holds
for that job**, against 14:30, 15:55 and lower before it, and it is the first
run on which the two Windows default lanes read within one second of each other.
Both are well inside 30:00 and neither is near the 24:00 review line. It is
recorded because the ARM lane's readings had been consistently below the x64
lane's and this run is where that stopped being true.

`pnpm-contracts` is the number that isolates this task's JavaScript cost: 0:47,
against `P2-U6`'s 0:50. `T161` adds no pnpm package and edits no `tools/*.mjs`
file; it edits three contract documents, which that job reads only through
`tools/policy-source-scan-inventory.test.mjs`'s inventory check.

**No job reaches the 80% review trigger on this head.** For the 30-minute Rust
jobs that line is 24:00. The crossing `P2-L2` recorded on a pre-rebase head
stands where that section records it, and nothing here discharges it.

## The `P2-U6` run

`P2-U6` adds one workspace member, `academic-ingestion`, and no pnpm workspace
package. Workspace membership and a default-workspace test are both triggers the
refresh rule names. The job count is unchanged at **17**: the new crate compiles
inside the `cargo test --workspace --locked` step the five `rust-default-*` jobs
already run, its `compile_fail` suite is one of that step's targets, and the two
`tools/*.test.mjs` files it edits already run inside `pnpm-contracts`. No new
job, no new step, no new feature lane, no system package.

The table is the head that merges. This branch was rebased twice — onto `P2-R1`,
and then onto `P2-L2`, which merged while the first of those runs was still
going — so the readings it took on its earlier heads sit on commits that are no
longer reachable and are not carried here. The reading immediately before this
one is [the `P2-L2` run](#the-p2-l2-run) above.

Run
[33744760718](https://github.com/dnhynk/Academic-Platform/actions/runs/33744760718)
completed **17/17 on `d8b58c5`**, first attempt, no rerun.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:05 | 5:00 | 1.7% |
| `rust-default-ubuntu-latest` | 6:35 | 30:00 | 21.9% |
| `rust-default-ubuntu-24.04-arm` | 5:05 | 30:00 | 16.9% |
| `rust-default-windows-latest` | 18:12 | 30:00 | 60.7% |
| `rust-default-windows-11-arm` | 14:30 | 30:00 | 48.3% |
| `rust-default-macos-latest` | 6:27 | 30:00 | 21.5% |
| `rust-features-ubuntu-latest` | 3:40 | 30:00 | 12.2% |
| `rust-features-ubuntu-24.04-arm` | 3:09 | 30:00 | 10.5% |
| `rust-features-windows-latest` | 5:15 | 30:00 | 17.5% |
| `rust-features-windows-11-arm` | 5:50 | 30:00 | 19.4% |
| `rust-features-macos-latest` | 3:48 | 30:00 | 12.7% |
| `phase1-exit-ubuntu-latest` | 4:14 | 45:00 | 9.4% |
| `phase1-exit-windows-latest` | 10:35 | 45:00 | 23.5% |
| `encrypted-store-lane-ubuntu-latest` | 3:29 | 45:00 | 7.7% |
| `encrypted-portability-lane-ubuntu-latest` | 5:22 | 45:00 | 11.9% |
| `rotation-orchestration-lane-ubuntu-latest` | 7:15 | 45:00 | 16.1% |
| `pnpm-contracts` | 0:50 | 15:00 | 5.6% |

**`rust-default-windows-latest` is the slowest job at 60.7%.** With `P2-R1`'s
16:56 restored to the list in the section above, this page now holds twelve
readings of that job: 12:16, 13:47, 14:08, 14:55, 15:28, 16:35, 16:56, 17:09,
18:03, 18:12, 18:41 and 20:18. This one is the third highest and sits inside the
band rather than extending it, so the guidance the `P2-L2` section gives is
unchanged: size headroom off 20:18, and read each further reading as evidence
about a range that is now 8:02 wide across trees that barely differ.

`phase1-exit-windows-latest` at 10:35 is the highest reading this page holds for
that job, against 8:10, 9:13 and 9:52 on the three runs before it. Its limit is
45:00, so 23.5% is not a budget question; it is recorded because it is the same
Windows-runner spread the row above shows, on a job whose readings had been
stable.

`pnpm-contracts` is the number that isolates this task's JavaScript cost, and it
went **down**: 0:50, against `P2-L2`'s 1:05 and `P2-R1`'s 0:58. `P2-U6` adds no
pnpm package and edits two files that job already runs — the same two files this
branch's conflicts were in, both times.

`encrypted-store-lane-ubuntu-latest` reads 7.7%. That lane is the one that pins
`STORE_MIGRATION_SQL` whole, so it is where a migration added to that set is
caught. `P2-U6` claims no migration and adds none, and `P2-R1`'s `0012` is
already inside the set this reading covers. The lane was run locally under WSL2
before every push to this branch.

**No job reaches the 80% review trigger on this head.** For the 30-minute Rust
jobs that line is 24:00. `P2-L2` recorded one crossing on a pre-rebase head;
that trigger stands where that section records it, and nothing here discharges
it.

### The `pull_request` event this branch's CI never received

The run in the table above was created by the pull request normally. An earlier
run on this branch was not, and that is worth recording because it is a second
thing on this repository's CI that is not a test result.

The branch's first two pushes each produced a `pull_request` run within about
twenty-five seconds. The third push, `bce9e47`, produced none: no run object was
created for that head at all, and `gh pr checks` reported *no checks reported on
the branch*. Closing and reopening the pull request — which emits `reopened`,
one of the `pull_request` trigger's default types — produced none either, twice,
over roughly ten minutes, while runs on `main` and on two other branches were
being created normally in the same window. `workflow_dispatch` on the same ref
produced a run immediately, on the same head SHA, materializing the same
seventeen jobs.

So the distinguishing observation is that the *event* was dropped, not that the
workflow failed: the same commit, the same workflow file and the same seventeen
jobs run green when the run is created by another trigger. A run that a webhook
never asked for is not a property of the change, exactly as a timeout
cancellation is not a test failure. If a branch shows no checks and the head SHA
has no run object, dispatch the workflow on that ref before looking for a cause
in the diff. `bce9e47` was rewritten by the later rebases and is no longer
reachable; the observation is kept because the rule it exercises is, and the two
runs are.

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
failure that survives the rerun is a test result.

### How often, and what the eight occurrences have in common

Counting it is what the sightings below could not do one at a time. Across the
57 runs of the split workflow, `rust-features-windows-latest` completed 56
attempts: **48 succeeded and 8 failed, and all 8 are this signature in this
step** — 14.3%, about one attempt in seven. No other post-split label has more
than one failure of any kind, and the one `rust-features-macos-latest` failure
was a compile gate, recorded above.

The eight logs agree on more than the signature:

- The failing test is **the second result `tests/containment.rs` reports**, at
  +0.44 s to +1.10 s, in all eight. The first is always
  `the_compiled_backend_is_the_one_this_platform_names`, which launches nothing.
- **Which** test it is varies with libtest's scheduling —
  `malicious_plugin_corpus_is_contained` five times,
  `resource_receipt_is_recorded_per_run` twice,
  `cpu_memory_time_output_limits_are_enforced` once, and on one occurrence the
  whole first wave of three failed together.
- Every later launch in the same binary succeeds, from the same absolute path,
  within seconds. Three of the eight tests call `Harness::baseline()` first,
  which launches that same image with `std::process::Command`; none of those
  three has ever been the failure.

**So it is the process's first sandboxed launch that fails, not a test and not a
missing file.** A binary that a plain `Command` starts, and that the same
process starts again 0.5 s later, is present and executable.

That is as far as the logs go, and it is less than a cause. What is one-time and
process-wide in `crates/worker/src/sandbox/windows.rs` is the AppContainer
setup: `container_sid` creates or derives one profile under a constant name, and
`grant` then read-modify-writes a DACL on the probe and on its parent directory
to add an allow ACE for that SID — shared state that concurrently scheduled
tests reach at the same moment, and an AppContainer that cannot open its image
is one documented way for `CreateProcessW` to answer `ERROR_FILE_NOT_FOUND`.
**That names a place to look, not a defect**; nothing here has reproduced it on
demand, and the experiment that would decide it is to serialize the first launch
and see whether the rate goes to zero. Until something does, do not change the
sandbox to make the symptom go away.

Seen on runs
[33696320874](https://github.com/dnhynk/Academic-Platform/actions/runs/33696320874),
[33697656939](https://github.com/dnhynk/Academic-Platform/actions/runs/33697656939)
(failed, then 17/17 on rerun of the one job),
[33715585336](https://github.com/dnhynk/Academic-Platform/actions/runs/33715585336)
in `P2-M2`,
[33716669384](https://github.com/dnhynk/Academic-Platform/actions/runs/33716669384),
[33718465338](https://github.com/dnhynk/Academic-Platform/actions/runs/33718465338),
[33730639197](https://github.com/dnhynk/Academic-Platform/actions/runs/33730639197),
[33739674269](https://github.com/dnhynk/Academic-Platform/actions/runs/33739674269)
and
[33740005635](https://github.com/dnhynk/Academic-Platform/actions/runs/33740005635),
and once locally on Windows in the `P2-G6` verification.

Run 33715585336 is the sighting where both halves of the rule were executed
against a tree that touches no part of
`academic-worker`. `resource_receipt_is_recorded_per_run`,
`cpu_memory_time_output_limits_are_enforced` and
`malicious_plugin_corpus_is_contained` failed together with the same
`CreateProcessW` launch error while the other five rows passed; the same suite
passed **8/8 on a Windows developer machine on that exact commit**, and
re-running only that job on that exact commit passed too. The commit under it
changed one test file in `crates/proposal` and one contract page, and the
identical job had already passed on the parent commit.

The **fourth** is `P2-L2`'s, recorded in [its own section](#the-p2-l2-run)
above. The **fifth** is `T159`'s, at `malicious_plugin_corpus_is_contained`
again, on run
[33739674269](https://github.com/dnhynk/Academic-Platform/actions/runs/33739674269)
— the first of that branch's two rebased heads, which a later rebase rewrote, so
the run is reachable and the commit is not. Both halves of the rule
were executed there too: the same-commit rerun of that one job passed, the
branch's delta touches no file under `crates/worker`, and that same test passes
on a Windows machine on that tree. Five hosted sightings, one job, one lane,
none surviving a rerun.

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

### The same signature on a developer machine, at a higher rate

`P2-L3` hit it outside hosted CI. Running
`cargo test -p academic-worker --all-targets --locked --offline --features native-sandbox`
on **Windows native**, on one commit, with no edit between attempts: **3 of 11
attempts failed**, and the two whose output was kept carry the signature
verbatim —

```text
Error: Launch { path: "...\target\debug\academic-worker-probe.exe",
                detail: "CreateProcessW returned 0 (last error 2)" }
```

— on seven of the eight `containment` rows at once, with `capability`'s thirteen
rows passing in the same run. The third failure is recorded as an exit code only:
the loop that found it reused one log file and the output was overwritten, which
is why the count above separates the two.

Two things this adds to the rows above. **`last error 2` is
`ERROR_FILE_NOT_FOUND`** against a path `cargo` had just built, which is what a
launch error rather than a sandbox refusal looks like, and it is the same reading
the hosted rows take. And **the rate is not a hosted-runner property**: 3 of 11
locally is higher than the 8 of 56 measured on `rust-features-windows-latest`,
on a machine with no runner contention. Neither number is a distribution; what
the pair establishes is that a same-commit rerun is the right response on either
side, and that a developer seeing it locally is not seeing something new.

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

## The `P2-U1` run

`P2-U1` adds one workspace member, `academic-curriculum`, and a canonical-store
migration, `0014`, both of which the refresh rule names as triggers. It also
adds a default-workspace test target — the new crate's three test binaries —
which is a third trigger. Run
[33777322136](https://github.com/dnhynk/Academic-Platform/actions/runs/33777322136)
at `c4f691f` completed **22/22**.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:04 | 5:00 | 1.3% |
| `phase1-exit-ubuntu-latest` | 4:14 | 45:00 | 9.4% |
| `pnpm-contracts` | 1:00 | 15:00 | 6.7% |
| `rust-store-windows-latest` | 6:09 | 30:00 | 20.5% |
| `rust-default-windows-11-arm` | 12:34 | 30:00 | 41.9% |
| `encrypted-store-lane-ubuntu-latest` | 3:27 | 45:00 | 7.7% |
| `encrypted-portability-lane-ubuntu-latest` | 5:20 | 45:00 | 11.9% |
| `rust-default-windows-latest` | 12:55 | 30:00 | 43.1% |
| `rust-default-ubuntu-24.04-arm` | 4:33 | 30:00 | 15.2% |
| `rust-default-macos-latest` | 4:47 | 30:00 | 15.9% |
| `rust-store-windows-11-arm` | 5:43 | 30:00 | 19.1% |
| `rust-store-ubuntu-24.04-arm` | 1:12 | 30:00 | 4.0% |
| `phase1-exit-windows-latest` | 9:28 | 45:00 | 21.0% |
| `rust-features-macos-latest` | 3:52 | 30:00 | 12.9% |
| `rotation-orchestration-lane-ubuntu-latest` | 7:09 | 45:00 | 15.9% |
| `rust-store-macos-latest` | 2:01 | 30:00 | 6.7% |
| `rust-store-ubuntu-latest` | 1:37 | 30:00 | 5.4% |
| `rust-features-windows-latest` | 6:45 | 30:00 | 22.5% |
| `rust-features-ubuntu-24.04-arm` | 3:00 | 30:00 | 10.0% |
| `rust-default-ubuntu-latest` | 5:15 | 30:00 | 17.5% |
| `rust-features-windows-11-arm` | 6:43 | 30:00 | 22.4% |
| `rust-features-ubuntu-latest` | 4:12 | 30:00 | 14.0% |

The slowest job is `rust-default-windows-latest` at 43.1%. That is the
**fourth** post-split reading of that job: 11:20, 12:43, 12:25 and now 12:55,
against the pre-split span the `P2-RF11` section holds. It is twelve seconds
above the largest of the three earlier readings, so it extends the range to
11:20–12:55 rather than sitting inside it — a range that is still under half the
30:00 limit. The guidance is unchanged: the 80% line for this job remains 24:00
and each further reading is evidence about the range rather than a replacement
for it. Its other half, `rust-store-windows-latest`, reads 6:09, which likewise
extends that job's range upward, to 5:13–6:09.

`rust-default-windows-11-arm` is beside it at 41.9%, a fourth reading in a
10:01–12:34 band. Every Linux, Linux ARM and macOS job is at or below 22.5%, and
no job on this run is above 43.1%.

The split's readings so far, as a range rather than a replacement:

| Label | Readings | Smallest | Largest |
|---|---:|---:|---:|
| `rust-default-windows-latest` | 4 | 11:20 | 12:55 |
| `rust-store-windows-latest` | 4 | 5:13 | 6:09 |
| `rust-default-windows-11-arm` | 4 | 10:01 | 12:34 |
| `rust-store-windows-11-arm` | 4 | 5:06 | 6:32 |

The new member adds one crate to `cargo clippy --workspace --all-targets` and
three test binaries — `curriculum`, `curriculum_scans` and a `trybuild` suite
with five cases — plus five in-crate store tests to `cargo test`. The trybuild
suite is the expensive one of the three: it compiles five programs against the
crate and its dev-dependencies, which is a second dependency resolution inside
the test run, and it lands on the `rust-default-*` jobs rather than the store or
feature ones.

`encrypted-store-lane-ubuntu-latest` is the job migration `0014` changes. `0014`
is the ninth entry in that lane's `STORE_MIGRATION_SQL` and the fifteen tables
it creates enter the creation transaction and the admission fingerprint.
## The `P2-R2` run

`P2-R2` adds one workspace member, `academic-repository-analysis`, which the
refresh rule names as a trigger. It adds no migration, so the encrypted-store
lane is unaffected by anything but the member itself. Run
[33760048834](https://github.com/dnhynk/Academic-Platform/actions/runs/33760048834)
at `5ac62b3` completed 22/22.

| Required job | Elapsed | Limit | Utilization |
|---|---:|---:|---:|
| `dependency-source-preflight` | 0:05 | 5:00 | 1.7% |
| `rust-default-ubuntu-latest` | 4:52 | 30:00 | 16.2% |
| `rust-default-ubuntu-24.04-arm` | 4:07 | 30:00 | 13.7% |
| `rust-default-windows-latest` | 13:12 | 30:00 | 44.0% |
| `rust-default-windows-11-arm` | 10:50 | 30:00 | 36.1% |
| `rust-default-macos-latest` | 6:14 | 30:00 | 20.8% |
| `rust-store-ubuntu-latest` | 1:33 | 30:00 | 5.2% |
| `rust-store-ubuntu-24.04-arm` | 1:14 | 30:00 | 4.1% |
| `rust-store-windows-latest` | 5:22 | 30:00 | 17.9% |
| `rust-store-windows-11-arm` | 5:10 | 30:00 | 17.2% |
| `rust-store-macos-latest` | 1:46 | 30:00 | 5.9% |
| `rust-features-ubuntu-latest` | 3:34 | 30:00 | 11.9% |
| `rust-features-ubuntu-24.04-arm` | 3:12 | 30:00 | 10.7% |
| `rust-features-windows-latest` | 6:26 | 30:00 | 21.4% |
| `rust-features-windows-11-arm` | 5:49 | 30:00 | 19.4% |
| `rust-features-macos-latest` | 4:06 | 30:00 | 13.7% |
| `phase1-exit-ubuntu-latest` | 4:06 | 45:00 | 9.1% |
| `phase1-exit-windows-latest` | 11:50 | 45:00 | 26.3% |
| `encrypted-store-lane-ubuntu-latest` | 3:24 | 45:00 | 7.6% |
| `encrypted-portability-lane-ubuntu-latest` | 4:45 | 45:00 | 10.6% |
| `rotation-orchestration-lane-ubuntu-latest` | 7:06 | 45:00 | 15.8% |
| `pnpm-contracts` | 1:01 | 15:00 | 6.8% |

The slowest job is `rust-default-windows-latest` at 44.0%. That is a **sixth**
reading of the job the `P2-RF11` section says has no single-reading budget:
20:18, 14:08, 12:16, 16:56, `P2-U1`'s 12:55 and now 13:12 — 67.7%, 47.1%, 40.9%,
56.4%, 43.1% and 44.0%. This branch was cut before `P2-U1` merged and rebased
onto it afterwards, so that reading is the one immediately above and this is the
next. It sits inside the range those five established rather than extending it,
so the guidance is unchanged: size headroom off 67.7% and treat each further
reading as evidence about the range. `phase1-exit-windows-latest` is the second
slowest at 26.3%, and every Linux, Linux ARM and macOS job is at or below 20.8%.

The new member adds one crate to `cargo clippy --workspace --all-targets` and
two test binaries — nineteen tests — to `cargo test --workspace`. It adds three
`compile_fail` cases to `academic-scenario`'s existing suite rather than a
fourth `compile_fail` target, so the two `rust-default-*` commands the README
lists are unchanged in number and `ci.yml` is untouched: the workflow still
materializes 22 required jobs.

### A second reading, at the branch head

The section above measures `5ac62b3`, the commit that adds the member. Four more
commits followed it — three repairs to this task's own source scan and one test
— so the default lane changed after that reading and the refresh rule names a
default-workspace test as a trigger. Run
[33779385348](https://github.com/dnhynk/Academic-Platform/actions/runs/33779385348)
at `c29a0a5` is the branch head, and it also completed 22/22. Both readings are
kept, for the reason this page gives everywhere else: a table is one reading of
a distribution, and the readings that decide a timeout are the ones no single
run shows.

| Required job | Elapsed | Utilization |
|---|---:|---:|
| `dependency-source-preflight` | 0:05 | 1.7% |
| `rust-default-ubuntu-latest` | 4:42 | 15.7% |
| `rust-default-ubuntu-24.04-arm` | 4:28 | 14.9% |
| `rust-default-windows-latest` | 11:48 | 39.3% |
| `rust-default-windows-11-arm` | 10:28 | 34.9% |
| `rust-default-macos-latest` | 5:26 | 18.1% |
| `rust-store-ubuntu-latest` | 1:24 | 4.7% |
| `rust-store-ubuntu-24.04-arm` | 1:23 | 4.6% |
| `rust-store-windows-latest` | 5:28 | 18.2% |
| `rust-store-windows-11-arm` | 5:19 | 17.7% |
| `rust-store-macos-latest` | 1:41 | 5.6% |
| `rust-features-ubuntu-latest` | 3:47 | 12.6% |
| `rust-features-ubuntu-24.04-arm` | 3:01 | 10.1% |
| `rust-features-windows-latest` | 7:50 | 26.1% |
| `rust-features-windows-11-arm` | 6:12 | 20.7% |
| `rust-features-macos-latest` | 4:02 | 13.4% |
| `phase1-exit-ubuntu-latest` | 3:48 | 8.4% |
| `phase1-exit-windows-latest` | 12:23 | 27.5% |
| `encrypted-store-lane-ubuntu-latest` | 3:08 | 7.0% |
| `encrypted-portability-lane-ubuntu-latest` | 4:57 | 11.0% |
| `rotation-orchestration-lane-ubuntu-latest` | 6:55 | 15.4% |
| `pnpm-contracts` | 0:51 | 5.7% |

`rust-default-windows-latest` is the slowest at 39.3%, a **seventh** reading of
that job — 20:18, 14:08, 12:16, 16:56, 12:55, 13:12 and now 11:47, which is
67.7%, 47.1%, 40.9%, 56.4%, 43.1%, 44.0% and 39.3%. It is the **lowest reading
so far**, so it extends the range's low end from 40.9% to 39.3% rather than
sitting inside it.
The guidance is unchanged for the reason the range exists: headroom is sized off
the *high* end, 67.7%, and that end did not move. A low reading is evidence about
the spread, not permission to shrink the timeout.

The four commits between the two readings add one test and change the text of a
test file, so nothing in the distribution should have moved; the 44.0% and 39.3%
readings differing by five points on near-identical work is the variance this
page records the range for.

Both readings predate this branch's rebase onto `P2-U1`, so neither includes
`academic-curriculum` in the default lane. The reading that does is `P2-U1`'s
own, two sections above; what this branch adds on top of it is one more crate
and nineteen more tests, and the next run on `main` is where the two appear
together.

Four runs between these two (`33776143099`, `33776572409`, `33778048462`,
`33778918470`) were cancelled by the workflow's own `cancel-in-progress`
concurrency group when the next commit superseded them. Per the refresh rule
those are not readings of anything and are excluded from both tables.
