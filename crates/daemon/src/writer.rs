//! One bounded admission lane owned by one dedicated operating-system thread.

use std::{
    collections::BTreeSet,
    fmt,
    path::PathBuf,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle, ThreadId},
};

use academic_core::local_service::{LocalService, LocalServiceError, LocalServiceStartup};
use academic_rpc::generated::MutableRequest;
use academic_store::profile::SyntheticProfile;
use thiserror::Error;
use tokio::sync::oneshot;

use crate::WRITER_QUEUE_CAPACITY;

type ServiceResult = Result<academic_rpc::generated::MutableResponse, LocalServiceError>;

#[derive(Debug)]
enum Work {
    Mutation {
        request: MutableRequest,
        reply: oneshot::Sender<ServiceResult>,
    },
}

/// A request was not admitted. No mutation occurred in either case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AdmissionError {
    /// All 64 pending slots were occupied at the instant of admission.
    #[error("writer queue is full")]
    ResourceExhausted,
    /// Graceful shutdown has begun or the writer thread failed.
    #[error("writer queue is shutting down")]
    ShuttingDown,
}

/// Receiver proving whether one admitted mutation completed.
#[derive(Debug)]
pub struct AdmittedMutation {
    receiver: oneshot::Receiver<ServiceResult>,
}

impl AdmittedMutation {
    /// Waits for the writer-owned acceptance attempt.
    pub async fn finish(self) -> Result<ServiceResult, AdmissionError> {
        self.receiver
            .await
            .map_err(|_| AdmissionError::ShuttingDown)
    }
}

/// The only mutable lane for a running profile.
pub struct WriterQueue {
    sender: Mutex<Option<SyncSender<Work>>>,
    join: Mutex<Option<JoinHandle<()>>>,
    accepting: AtomicBool,
    current_revision: AtomicU64,
    owner_thread: ThreadId,
    _registration: WriterRegistration,
}

impl fmt::Debug for WriterQueue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriterQueue")
            .field("capacity", &WRITER_QUEUE_CAPACITY)
            .field("accepting", &self.accepting.load(Ordering::Acquire))
            .field("current_revision", &self.current_revision())
            .field("owner_thread", &self.owner_thread)
            .field("database_path", &self._registration.database_path)
            .finish_non_exhaustive()
    }
}

impl WriterQueue {
    /// Starts the dedicated writer thread and blocks until V1 reconciliation
    /// and the sole `WriterConnection` have been established on that thread.
    pub fn start(
        profile: SyntheticProfile,
    ) -> Result<(Self, LocalServiceStartup), LocalServiceError> {
        let registration = WriterRegistration::claim(&profile)?;
        let (sender, receiver) = mpsc::sync_channel::<Work>(WRITER_QUEUE_CAPACITY);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("academicd-profile-writer".to_owned())
            .spawn(move || {
                let owner_thread = thread::current().id();
                let (mut service, startup) =
                    match LocalService::open(profile, std::time::SystemTime::now()) {
                        Ok(value) => value,
                        Err(error) => {
                            let _ignored = ready_sender.send(Err(error));
                            return;
                        }
                    };
                let revision = startup.profile_revision();
                if ready_sender
                    .send(Ok((startup, owner_thread, revision)))
                    .is_err()
                {
                    return;
                }
                while let Ok(work) = receiver.recv() {
                    match work {
                        Work::Mutation { request, reply } => {
                            let result = service.handle_mutable_request_now(&request);
                            let _ignored = reply.send(result);
                        }
                    }
                }
            })
            .map_err(|_| {
                LocalServiceError::UnexpectedCanonicalState("writer thread did not start")
            })?;
        let (startup, owner_thread, revision) = match ready_receiver.recv() {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                let _ignored = join.join();
                return Err(error);
            }
            Err(_) => {
                let _ignored = join.join();
                return Err(LocalServiceError::UnexpectedCanonicalState(
                    "writer thread stopped before readiness",
                ));
            }
        };
        Ok((
            Self {
                sender: Mutex::new(Some(sender)),
                join: Mutex::new(Some(join)),
                accepting: AtomicBool::new(true),
                current_revision: AtomicU64::new(revision),
                owner_thread,
                _registration: registration,
            },
            startup,
        ))
    }

    /// Attempts admission without waiting for queue capacity.
    pub fn try_admit(&self, request: MutableRequest) -> Result<AdmittedMutation, AdmissionError> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err(AdmissionError::ShuttingDown);
        }
        let (reply, receiver) = oneshot::channel();
        let guard = self
            .sender
            .lock()
            .map_err(|_| AdmissionError::ShuttingDown)?;
        let sender = guard.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        match sender.try_send(Work::Mutation { request, reply }) {
            Ok(()) => Ok(AdmittedMutation { receiver }),
            Err(TrySendError::Full(_)) => Err(AdmissionError::ResourceExhausted),
            Err(TrySendError::Disconnected(_)) => Err(AdmissionError::ShuttingDown),
        }
    }

    /// Returns the most recently completed canonical revision observed by the
    /// transport. It is monotonic and used only for explicit denial receipts.
    #[must_use]
    pub fn current_revision(&self) -> u64 {
        self.current_revision.load(Ordering::Acquire)
    }

    /// Records a successfully completed response revision.
    pub fn observe_revision(&self, revision: u64) {
        self.current_revision.fetch_max(revision, Ordering::AcqRel);
    }

    /// Returns the opaque identity of the OS thread that owns the writer.
    #[must_use]
    pub const fn owner_thread(&self) -> ThreadId {
        self.owner_thread
    }

    /// Stops admission, drains already-admitted work, and joins the writer.
    pub fn shutdown(&self) {
        self.accepting.store(false, Ordering::Release);
        if let Ok(mut sender) = self.sender.lock() {
            let _dropped = sender.take();
        }
        if let Ok(mut join) = self.join.lock()
            && let Some(join) = join.take()
        {
            let _ignored = join.join();
        }
    }
}

static WRITER_REGISTRY: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();

#[derive(Debug)]
struct WriterRegistration {
    database_path: PathBuf,
}

impl WriterRegistration {
    fn claim(profile: &SyntheticProfile) -> Result<Self, LocalServiceError> {
        let database_path = std::fs::canonicalize(profile.database_path()).map_err(|_| {
            LocalServiceError::UnexpectedCanonicalState("writer database identity is unavailable")
        })?;
        let mut registry = WRITER_REGISTRY
            .get_or_init(|| Mutex::new(BTreeSet::new()))
            .lock()
            .map_err(|_| {
                LocalServiceError::UnexpectedCanonicalState("writer registry is unavailable")
            })?;
        if !registry.insert(database_path.clone()) {
            return Err(LocalServiceError::UnexpectedCanonicalState(
                "writer connection is already owned in this process",
            ));
        }
        Ok(Self { database_path })
    }
}

impl Drop for WriterRegistration {
    fn drop(&mut self) {
        if let Ok(mut registry) = WRITER_REGISTRY
            .get_or_init(|| Mutex::new(BTreeSet::new()))
            .lock()
        {
            let _removed = registry.remove(&self.database_path);
        }
    }
}

impl Drop for WriterQueue {
    fn drop(&mut self) {
        self.accepting.store(false, Ordering::Release);
        if let Ok(sender) = self.sender.get_mut() {
            let _dropped = sender.take();
        }
        if let Ok(join) = self.join.get_mut()
            && let Some(join) = join.take()
        {
            let _ignored = join.join();
        }
    }
}
