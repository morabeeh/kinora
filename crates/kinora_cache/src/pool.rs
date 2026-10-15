//! A small pool of background worker threads with **priorities** and **cancellation**
//! (brief §7.5). Think of it as a `BlockingCollection` drained by N threads, except that the
//! most urgent job always goes first and jobs nobody needs any more are dropped before
//! they start.
//!
//! Each job has a key (e.g. the file path plus the requested size). When the user moves
//! on, the app calls [`WorkPool::reprioritize`] to drop jobs that are no longer wanted and
//! re-rank the rest — "cancel stale work immediately when the user skips quickly".
//! A job that is already running finishes; its result is recognised as stale by the app.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

type Work = Box<dyn FnOnce() + Send + 'static>;
type PanicHandler<K> = Box<dyn Fn(&K) + Send + Sync + 'static>;

struct Queued<K> {
    key: K,
    priority: u8,
    sequence: u64,
    work: Work,
}

impl<K> PartialEq for Queued<K> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl<K> Eq for Queued<K> {}
impl<K> PartialOrd for Queued<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<K> Ord for Queued<K> {
    /// `BinaryHeap` pops the *largest* item, so "larger" here means more urgent: a lower
    /// priority number, then the earlier submission.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

struct State<K> {
    queue: BinaryHeap<Queued<K>>,
    running: Vec<K>,
    sequence: u64,
    shutting_down: bool,
}

struct Shared<K> {
    state: Mutex<State<K>>,
    wake: Condvar,
    on_panic: PanicHandler<K>,
}

impl<K> Shared<K> {
    /// Locks the state. A poisoned lock (a panic while holding it) can't leave the queue
    /// inconsistent — every update is a single push/pop — so we keep going.
    fn lock(&self) -> MutexGuard<'_, State<K>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Background workers that run the most urgent job first.
pub struct WorkPool<K: Clone + PartialEq + Send + 'static> {
    shared: Arc<Shared<K>>,
    workers: Vec<JoinHandle<()>>,
}

impl<K: Clone + PartialEq + Send + 'static> std::fmt::Debug for WorkPool<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkPool")
            .field("workers", &self.workers.len())
            .finish_non_exhaustive()
    }
}

impl<K: Clone + PartialEq + Send + 'static> WorkPool<K> {
    /// Starts `threads` workers (at least one) named `name-0`, `name-1`, …
    /// `on_panic` is called with the key of a job that panicked (the panic itself is caught,
    /// so the worker keeps running).
    ///
    /// # Errors
    /// If the OS refuses to create a thread.
    pub fn new(
        name: &str,
        threads: usize,
        on_panic: impl Fn(&K) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                queue: BinaryHeap::new(),
                running: Vec::new(),
                sequence: 0,
                shutting_down: false,
            }),
            wake: Condvar::new(),
            on_panic: Box::new(on_panic),
        });
        let workers = (0..threads.max(1))
            .map(|index| {
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name(format!("{name}-{index}"))
                    .spawn(move || worker(&shared))
            })
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(Self { shared, workers })
    }

    /// Queues a job. Lower `priority` numbers run first (0 = the item on screen).
    pub fn submit(&self, key: K, priority: u8, work: impl FnOnce() + Send + 'static) {
        let mut state = self.shared.lock();
        state.sequence += 1;
        let sequence = state.sequence;
        state.queue.push(Queued {
            key,
            priority,
            sequence,
            work: Box::new(work),
        });
        drop(state);
        self.shared.wake.notify_one();
    }

    /// Re-ranks every queued job: `rank(key)` returns its new priority, or `None` to drop
    /// it without running. Running jobs are not affected.
    pub fn reprioritize(&self, mut rank: impl FnMut(&K) -> Option<u8>) {
        let mut state = self.shared.lock();
        let jobs = std::mem::take(&mut state.queue).into_vec();
        state.queue = jobs
            .into_iter()
            .filter_map(|mut job| {
                job.priority = rank(&job.key)?;
                Some(job)
            })
            .collect();
    }

    /// `true` if a job with this key is queued or running (so it needn't be submitted again).
    #[must_use]
    pub fn is_pending(&self, key: &K) -> bool {
        let state = self.shared.lock();
        state.running.contains(key) || state.queue.iter().any(|job| job.key == *key)
    }

    /// Number of jobs waiting to start.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.shared.lock().queue.len()
    }
}

impl<K: Clone + PartialEq + Send + 'static> Drop for WorkPool<K> {
    /// Drops queued jobs, lets running ones finish, and joins the threads.
    fn drop(&mut self) {
        {
            let mut state = self.shared.lock();
            state.shutting_down = true;
            state.queue.clear();
        }
        self.shared.wake.notify_all();
        for worker in self.workers.drain(..) {
            // A worker can only end by returning; nothing useful to do with an error here.
            let _ = worker.join();
        }
    }
}

fn worker<K: Clone + PartialEq + Send>(shared: &Shared<K>) {
    loop {
        let job = {
            let mut state = shared.lock();
            loop {
                if state.shutting_down {
                    return;
                }
                if let Some(job) = state.queue.pop() {
                    state.running.push(job.key.clone());
                    break job;
                }
                state = shared
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let Queued { key, work, .. } = job;
        // One bad job (e.g. a decoder bug) must not take the worker or the app down.
        if catch_unwind(AssertUnwindSafe(work)).is_err() {
            tracing::error!("a background job panicked; the worker keeps running");
            (shared.on_panic)(&key);
        }
        let mut state = shared.lock();
        if let Some(index) = state.running.iter().position(|running| *running == key) {
            state.running.swap_remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A pool whose single worker is blocked until `release` is sent, so jobs can be queued
    /// up and their order observed.
    fn blocked_pool() -> (WorkPool<u32>, mpsc::Sender<()>) {
        let pool = WorkPool::new("test", 1, |_| {}).unwrap();
        let (release, gate) = mpsc::channel::<()>();
        pool.submit(0, 0, move || {
            let _ = gate.recv();
        });
        // Wait until the blocker is running.
        while pool.queued() > 0 {
            std::thread::yield_now();
        }
        (pool, release)
    }

    #[test]
    fn most_urgent_job_runs_first_then_fifo() {
        let (pool, release) = blocked_pool();
        let (done, results) = mpsc::channel();
        for (key, priority) in [(1, 5), (2, 1), (3, 5), (4, 0)] {
            let done = done.clone();
            pool.submit(key, priority, move || done.send(key).unwrap());
        }
        release.send(()).unwrap();
        let order: Vec<u32> = (0..4)
            .map(|_| results.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        assert_eq!(order, [4, 2, 1, 3]);
    }

    #[test]
    fn reprioritize_drops_and_reorders() {
        let (pool, release) = blocked_pool();
        let (done, results) = mpsc::channel();
        for key in [1, 2, 3] {
            let done = done.clone();
            pool.submit(key, 1, move || done.send(key).unwrap());
        }
        assert!(pool.is_pending(&2));
        // The user moved on: 1 is stale, 3 became urgent.
        pool.reprioritize(|key| match key {
            1 => None,
            3 => Some(0),
            other => u8::try_from(*other).ok(),
        });
        assert!(!pool.is_pending(&1));
        release.send(()).unwrap();
        let first = results.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((first, second), (3, 2));
        assert!(results.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn a_panicking_job_does_not_kill_the_pool() {
        let (panicked, panics) = mpsc::channel();
        let pool = WorkPool::new("test", 1, move |key: &u32| {
            panicked.send(*key).unwrap();
        })
        .unwrap();
        pool.submit(7, 0, || panic!("decoder bug"));
        let (done, results) = mpsc::channel();
        pool.submit(8, 0, move || done.send(8).unwrap());
        assert_eq!(panics.recv_timeout(Duration::from_secs(5)).unwrap(), 7);
        assert_eq!(results.recv_timeout(Duration::from_secs(5)).unwrap(), 8);
        assert!(!pool.is_pending(&7));
    }

    #[test]
    fn dropping_the_pool_joins_its_threads() {
        let pool = WorkPool::new("test", 3, |_: &u32| {}).unwrap();
        for key in 0..10 {
            pool.submit(key, 1, || std::thread::sleep(Duration::from_millis(1)));
        }
        drop(pool); // must not hang
    }
}
