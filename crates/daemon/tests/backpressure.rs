use std::{
    error::Error,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use academic_daemon::{AdmissionError, RunningDaemon, WRITER_QUEUE_CAPACITY, WriterQueue};
use academic_rpc::generated::MutationStatus;
use academic_store::queries::canonical_snapshot;

pub mod support;

use support::{TestEnvironment, client_exchange, request};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_queue_serializes_acceptance() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let duplicate_profile = profile.clone();
    let (queue, _) = WriterQueue::start(profile)?;
    assert_ne!(queue.owner_thread(), std::thread::current().id());
    assert!(WriterQueue::start(duplicate_profile).is_err());
    let first = queue.try_admit(request(1, Some(0))?)?;
    let second = queue.try_admit(request(2, Some(0))?)?;
    let first = first.finish().await??;
    let second = second.finish().await??;
    assert_eq!(first.status, MutationStatus::Accepted as i32);
    assert_eq!(second.status, MutationStatus::Rejected as i32);
    assert_eq!(second.reason, "REVISION_CONFLICT");
    queue.shutdown();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_queue_saturation_is_backpressure_not_drop() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let queue = daemon.writer();
    let stop_pressure = Arc::new(AtomicBool::new(false));
    let pressure_stop = Arc::clone(&stop_pressure);
    let pressure_requests = (1_u8..=192)
        .map(|seed| request(seed, None))
        .collect::<Result<Vec<_>, _>>()?;
    let (full_sender, full_receiver) = tokio::sync::oneshot::channel();
    let pressure = tokio::spawn(async move {
        let mut full_sender = Some(full_sender);
        let mut admitted = Vec::new();
        let mut next = 0_usize;
        while !pressure_stop.load(Ordering::Acquire) {
            match queue.try_admit(pressure_requests[next].clone()) {
                Ok(mutation) => {
                    admitted.push(mutation);
                    next = (next + 1) % pressure_requests.len();
                }
                Err(AdmissionError::ResourceExhausted) => {
                    if let Some(sender) = full_sender.take() {
                        let _ignored = sender.send(());
                    }
                    tokio::task::yield_now().await;
                }
                Err(AdmissionError::ShuttingDown) => break,
            }
        }
        admitted
    });
    full_receiver.await?;

    let mut rejected_request = None;
    for seed in 201_u8..=232 {
        let candidate = request(seed, None)?;
        let response =
            client_exchange(daemon.endpoint(), daemon.session_nonce(), candidate.clone()).await?;
        if response.status == MutationStatus::Rejected as i32
            && response.reason == "RESOURCE_EXHAUSTED"
        {
            rejected_request = Some(candidate);
            break;
        }
    }
    stop_pressure.store(true, Ordering::Release);
    let admitted = pressure.await?;
    assert!(admitted.len() >= WRITER_QUEUE_CAPACITY);
    for mutation in admitted {
        assert!(mutation.finish().await?.is_ok());
    }

    let rejected_request = rejected_request
        .ok_or_else(|| io::Error::other("real IPC admission never reported saturation"))?;
    let before_retry = canonical_snapshot(&profile.open_reader()?)?;
    let retry =
        client_exchange(daemon.endpoint(), daemon.session_nonce(), rejected_request).await?;
    // The allowlisted signed batch already exists because pressure admissions
    // use the same frozen fixture, so a new request receipt is a batch-level
    // duplicate while still proving this request was never admitted before.
    assert_eq!(retry.status, MutationStatus::Duplicate as i32);
    let after_retry = canonical_snapshot(&profile.open_reader()?)?;
    assert_eq!(after_retry.receipt_count, before_retry.receipt_count + 1);
    daemon.shutdown().await?;
    Ok(())
}
