//! Bounded background conversion worker shared by the desktop UI.

use pixelforge_core::{ConversionOptions, ConversionReport, OutputFormat, convert_file};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobState {
    Pending,
    Validating,
    Processing,
    Completed,
    Skipped,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Skipped | Self::Failed | Self::Cancelled
        )
    }
}

#[derive(Debug, Clone)]
pub struct BatchRequest {
    pub inputs: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub formats: Vec<OutputFormat>,
    pub options: ConversionOptions,
}

#[derive(Debug)]
pub enum WorkerEvent {
    BatchStarted {
        total: usize,
    },
    ItemStarted {
        input: PathBuf,
        format: OutputFormat,
        completed: usize,
        total: usize,
    },
    ItemFinished {
        input: PathBuf,
        format: OutputFormat,
        result: Result<ConversionReport, String>,
        completed: usize,
        total: usize,
    },
    BatchFinished {
        succeeded: usize,
        failed: usize,
        cancelled: bool,
    },
}

#[derive(Debug, Error)]
pub enum SubmitError {
    #[error("the conversion queue is full; wait for the active batch to finish")]
    Full,
    #[error("the conversion worker has stopped")]
    Stopped,
    #[error("choose at least one input image and one output format")]
    Empty,
}

enum Message {
    Run(BatchRequest),
}

pub struct Worker {
    sender: SyncSender<Message>,
    events: Receiver<WorkerEvent>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

impl Worker {
    /// Starts one dedicated worker and a one-batch bounded command queue.
    pub fn new() -> Self {
        let (sender, requests) = mpsc::sync_channel::<Message>(1);
        let (event_tx, events) = mpsc::channel();
        let paused = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_paused = Arc::clone(&paused);
        let worker_cancelled = Arc::clone(&cancelled);
        thread::Builder::new()
            .name("pixelforge-converter".into())
            .spawn(move || {
                while let Ok(Message::Run(batch)) = requests.recv() {
                    let total = batch.inputs.len().saturating_mul(batch.formats.len());
                    let _ = event_tx.send(WorkerEvent::BatchStarted { total });
                    let mut completed = 0;
                    let mut succeeded = 0;
                    let mut failed = 0;
                    'batch: for input in &batch.inputs {
                        for format in &batch.formats {
                            while worker_paused.load(Ordering::Acquire)
                                && !worker_cancelled.load(Ordering::Acquire)
                            {
                                thread::sleep(Duration::from_millis(80));
                            }
                            if worker_cancelled.load(Ordering::Acquire) {
                                break 'batch;
                            }
                            let _ = event_tx.send(WorkerEvent::ItemStarted {
                                input: input.clone(),
                                format: *format,
                                completed,
                                total,
                            });
                            let result =
                                convert_file(input, &batch.output_dir, *format, &batch.options)
                                    .map_err(|error| error.to_string());
                            if result.is_ok() {
                                succeeded += 1;
                            } else {
                                failed += 1;
                            }
                            completed += 1;
                            let _ = event_tx.send(WorkerEvent::ItemFinished {
                                input: input.clone(),
                                format: *format,
                                result,
                                completed,
                                total,
                            });
                        }
                    }
                    let was_cancelled = worker_cancelled.swap(false, Ordering::AcqRel);
                    worker_paused.store(false, Ordering::Release);
                    let _ = event_tx.send(WorkerEvent::BatchFinished {
                        succeeded,
                        failed,
                        cancelled: was_cancelled,
                    });
                }
            })
            .expect("could not start the PixelForge conversion worker");
        Self {
            sender,
            events,
            paused,
            cancelled,
        }
    }

    pub fn submit(&self, batch: BatchRequest) -> Result<(), SubmitError> {
        if batch.inputs.is_empty() || batch.formats.is_empty() {
            return Err(SubmitError::Empty);
        }
        self.cancelled.store(false, Ordering::Release);
        self.paused.store(false, Ordering::Release);
        self.sender
            .try_send(Message::Run(batch))
            .map_err(|error| match error {
                TrySendError::Full(_) => SubmitError::Full,
                TrySendError::Disconnected(_) => SubmitError::Stopped,
            })
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }
    pub fn cancel(&self) {
        self.paused.store(false, Ordering::Release);
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn try_recv(&self) -> Result<WorkerEvent, mpsc::TryRecvError> {
        self.events.try_recv()
    }
}

impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn job_state_terminal_set_is_explicit() {
        assert!(!JobState::Pending.terminal());
        assert!(!JobState::Processing.terminal());
        assert!(JobState::Completed.terminal());
        assert!(JobState::Failed.terminal());
        assert!(JobState::Cancelled.terminal());
    }

    #[test]
    fn empty_batches_are_rejected() {
        let worker = Worker::new();
        let batch = BatchRequest {
            inputs: Vec::new(),
            output_dir: PathBuf::new(),
            formats: vec![OutputFormat::Png],
            options: ConversionOptions::default(),
        };
        assert!(matches!(worker.submit(batch), Err(SubmitError::Empty)));
    }

    #[test]
    fn mixed_batch_supports_multiple_outputs_and_continues_after_failure() {
        let root = std::env::temp_dir().join(format!(
            "pixelforge-jobs-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let png = root.join("alpha.png");
        let jpeg = root.join("beta.jpeg");
        let broken = root.join("broken.data");
        image::RgbImage::new(5, 4).save(&png).unwrap();
        image::RgbImage::new(6, 3).save(&jpeg).unwrap();
        fs::write(&broken, b"not an image").unwrap();

        let worker = Worker::new();
        worker
            .submit(BatchRequest {
                inputs: vec![png, jpeg, broken],
                output_dir: root.join("out"),
                formats: vec![OutputFormat::Png, OutputFormat::Webp],
                options: ConversionOptions::default(),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut summary = None;
        while Instant::now() < deadline {
            match worker.try_recv() {
                Ok(WorkerEvent::BatchFinished {
                    succeeded,
                    failed,
                    cancelled,
                }) => {
                    summary = Some((succeeded, failed, cancelled));
                    break;
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(10)),
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(summary, Some((4, 2, false)));
        assert!(root.join("out/alpha.png").exists());
        assert!(root.join("out/alpha.webp").exists());
        assert!(root.join("out/beta.png").exists());
        assert!(root.join("out/beta.webp").exists());
        let _ = fs::remove_dir_all(root);
    }
}
