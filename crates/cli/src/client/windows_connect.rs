//! Windows availability retries, exclusively before a stream is returned.

use std::{
    future::Future,
    io,
    time::{Duration, Instant},
};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

const MAX_ATTEMPTS: usize = 25;
const DEADLINE: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_millis(20);

pub(super) async fn connect(name: &str) -> io::Result<NamedPipeClient> {
    open_with_retry(
        || ClientOptions::new().open(name),
        Instant::now,
        tokio::time::sleep,
    )
    .await
}

// The injected operations let the regression drive the exact clock and native
// open outcomes without scheduling races or a test-only product retry policy.
async fn open_with_retry<T, S: Future<Output = ()>>(
    mut open: impl FnMut() -> io::Result<T>,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration) -> S,
) -> io::Result<T> {
    let deadline = now() + DEADLINE;
    let mut attempts = 0;
    loop {
        attempts += 1;
        let error = match open() {
            Ok(stream) => return Ok(stream),
            Err(error) => error,
        };
        // Win32 ERROR_FILE_NOT_FOUND / ERROR_PIPE_BUSY only. BrokenPipe,
        // access denied and all other failures must return immediately.
        if !matches!(error.raw_os_error(), Some(2 | 231)) || attempts >= MAX_ATTEMPTS {
            return Err(error);
        }
        let remaining = deadline.saturating_duration_since(now());
        if remaining.is_zero() {
            return Err(error);
        }
        sleep(RETRY_DELAY.min(remaining)).await;
        // Do not make a final open after the deadline if scheduling overshoots.
        if now() >= deadline {
            return Err(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[tokio::test]
    async fn retries_only_the_two_pre_connection_availability_errors() -> io::Result<()> {
        for code in [2, 231, 5, 109, 232, 87] {
            let attempts = Cell::new(0);
            let clock = Cell::new(Instant::now());
            let result = open_with_retry(
                || {
                    attempts.set(attempts.get() + 1);
                    if attempts.get() == 1 {
                        Err(io::Error::from_raw_os_error(code))
                    } else {
                        Ok(())
                    }
                },
                || clock.get(),
                |delay| {
                    clock.set(clock.get() + delay);
                    std::future::ready(())
                },
            )
            .await;
            if [2, 231].contains(&code) {
                result?;
                assert_eq!(attempts.get(), 2);
            } else {
                assert_eq!(
                    result.err().and_then(|error| error.raw_os_error()),
                    Some(code)
                );
                assert_eq!(attempts.get(), 1);
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn attempts_and_overall_deadline_are_independent_bounds() {
        for elapsed_per_sleep in [
            Duration::ZERO,
            Duration::from_millis(200),
            Duration::from_millis(499),
            DEADLINE,
        ] {
            let attempts = Cell::new(0);
            let clock = Cell::new(Instant::now());
            let waits = Cell::new(Duration::ZERO);
            let result: io::Result<()> = open_with_retry(
                || {
                    attempts.set(attempts.get() + 1);
                    Err(io::Error::from_raw_os_error(231))
                },
                || clock.get(),
                |delay| {
                    waits.set(waits.get() + delay);
                    clock.set(clock.get() + elapsed_per_sleep);
                    std::future::ready(())
                },
            )
            .await;
            assert_eq!(
                result.err().and_then(|error| error.raw_os_error()),
                Some(231)
            );
            assert_eq!(
                attempts.get(),
                match elapsed_per_sleep.as_millis() {
                    0 => 25,
                    200 => 3,
                    499 => 2,
                    _ => 1,
                }
            );
            assert!(waits.get() <= DEADLINE);
            if elapsed_per_sleep == Duration::from_millis(499) {
                assert_eq!(waits.get(), Duration::from_millis(21));
            }
        }
    }
}
