use std::process::ExitCode;

use academic_policy::ProcessClass;

const PROCESS_CLASS: ProcessClass = ProcessClass::CaptureClient;

fn main() -> ExitCode {
    match academic_process_sandbox::enter(PROCESS_CLASS) {
        Ok(enforcement) => {
            println!("{}", enforcement.receipt_line());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "{}",
                academic_process_sandbox::refusal_line(PROCESS_CLASS, &error)
            );
            ExitCode::FAILURE
        }
    }
}
