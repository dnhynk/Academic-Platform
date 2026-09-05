use academic_policy::ProcessClass;

const PROCESS_CLASS: ProcessClass = ProcessClass::CaptureClient;

// The whole of `main`. `P2-A5`'s sixth audit put a live name resolution above
// the sandbox entry in a hand-written `main` here and measured no difference
// anywhere in the workspace; this crate now declares no `fn main` at all, so
// there is no statement position above the entry to write one into.
academic_process_sandbox::class_main!(PROCESS_CLASS);
