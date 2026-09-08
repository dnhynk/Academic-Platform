// Ordinary synthetic encrypted correctness only; no admission/signing commands.
import { posix, win32 } from "node:path";

export const evidencePath = (target) => PLATFORMS[target][1] === "win32" ? win32 : posix;

export const PERL_IDENTITY_ARGS = ["-e", "print $^V, qq(\\n), $Config::Config{archname}, qq(\\n)", "-MConfig"];
export const PERL_MODULE_ARGS = ["-e", "use Locale::Maketext::Simple; use Params::Check; use IPC::Cmd; use Pod::Usage; print qq(ok\\n)"];

export function prerequisitePlan(target, execution, pin) {
  const windows = PLATFORMS[target][1] === "win32";
  return [
    ...(windows ? [{ id: "windows-prerequisites", executable: "pwsh", args: ["-NoProfile", "-File", "tools/h1-prerequisites.ps1"] }] : []),
    { id: "node", executable: execution.nodeExecutable, args: ["--version"] },
    { id: "rustc", executable: "rustc", args: ["-vV"] },
    { id: "cargo", executable: "cargo", args: ["--version"] },
    { id: "perl", executable: windows ? evidencePath(target).join(execution.runnerTemp, "h1-perl", pin.perl_relative_path) : "perl", args: ["-V"] },
    ...(windows ? [{ id: "windows-toolchain", executable: execution.nodeExecutable, args: ["tools/h1-windows-toolchain.mjs"] }] : [
      { id: "cc", executable: "cc", args: ["--version"] },
      { id: "make", executable: "make", args: ["--version"] },
    ]),
    { id: "fetch", executable: "cargo", args: ["fetch", "--locked"] },
  ];
}
export const PLATFORMS = {
  "windows-x86_64": ["windows-latest", "win32", "x64", "x86_64-pc-windows-msvc"],
  "windows-aarch64": ["windows-11-arm", "win32", "arm64", "aarch64-pc-windows-msvc"],
  "linux-x86_64": ["ubuntu-latest", "linux", "x64", "x86_64-unknown-linux-gnu"],
  "linux-aarch64": ["ubuntu-24.04-arm", "linux", "arm64", "aarch64-unknown-linux-gnu"],
  "macos-aarch64": ["macos-latest", "darwin", "arm64", "aarch64-apple-darwin"],
};

export const CATEGORIES = [
  "hosted_windows_phase2_exit", "hosted_linux_phase2_exit",
  "platform_build_and_license_receipt", "platform_zero_canary",
  "platform_fault_and_restore", "platform_keystore_native",
  "five_platform_receipt_is_complete", "missing_platform_keeps_admission_denied",
];

export function commandPlan() {
  const store = ["-p", "academic-store", "--no-default-features", "--features", "sqlcipher-store", "--locked", "--offline"];
  const portability = ["-p", "academic-portability", "--no-default-features", "--features", "encrypted-portability", "--locked", "--offline"];
  return [
    { id: "store-lint", args: ["clippy", ...store, "--all-targets", "--", "-D", "warnings"] },
    { id: "store-tests", args: ["test", ...store, "--message-format=json", "--", "--test-threads=1"], requiredTests: [
      "encrypted::wrong_store_key_fails_closed", "encrypted::zero_canary_in_db_wal_shm_temp_backup_crash",
      "encrypted::store_rekey_kill_leaves_exactly_one_working_key", "encrypted::db_faults_replay_under_the_cipher_lane",
    ] },
    { id: "probe-build", args: ["build", ...store, "--bin", "sqlcipher_store_probe", "--message-format=json"] },
    { id: "portability-lint", args: ["clippy", ...portability, "--all-targets", "--", "-D", "warnings"] },
    { id: "portability-tests", args: ["test", ...portability, "--message-format=json", "--", "--test-threads=1"], requiredTests: [
      "fresh_machine_restore_with_phrase_only", "restore_verifies_ledger_object_and_count_closure",
      "a_backed_up_database_is_unreadable_without_its_key",
    ] },
    { id: "encrypted-crash", args: ["test", "-p", "academic-portability", "--no-default-features", "--features", "encrypted-portability,phase2-fault-injection", "--locked", "--offline", "--test", "encrypted_crash", "--message-format=json", "--", "--test-threads=1"], requiredTests: [
      "bk01_bk04_leave_no_partially_published_encrypted_backup", "rs01_rs04_leave_no_partially_activated_encrypted_profile",
    ] },
  ];
}
