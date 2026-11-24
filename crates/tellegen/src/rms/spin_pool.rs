//! Experimental: a small pool of threads that wait by spinning, for
//! the fine-grained parallel parts of a step (the units' residual and
//! Jacobian). A step hands out work thousands of times a second; a pool whose
//! threads sleep between jobs (rayon) pays a kernel wake-up each time, which
//! costs more than the work. Here a job is a function of a task index; the
//! caller works on tasks too and returns once every task is done.
//!
//! `TELLEGEN_THREADS` sets the number of threads besides the caller (default 3;
//! 0 runs everything on the caller). Idle threads spin briefly, then yield,
//! then sleep in short naps, so a pool between runs costs little.
//!
//! In the browser (a WebAssembly build with shared memory) the adapter starts
//! the threads as Web Workers through [`configure`], before the first run; a
//! build or page without them runs everything on the caller.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use crate::rms::clock::Instant;

type Job = dyn Fn(usize) + Sync + 'static;

/// Starts a thread that runs the closure (the browser adapter's Web Worker).
pub type Spawner = fn(Box<dyn FnOnce() + Send>);

#[cfg(target_arch = "wasm32")]
static SPAWNER: OnceLock<Spawner> = OnceLock::new();
/// The threads of the step's pool and of the relays' pool, when set.
static COUNTS: OnceLock<(usize, usize)> = OnceLock::new();

/// The thread counts of the step's pool and the relays' pool besides the
/// caller, and (in the browser) how a thread is started; then starts the
/// pools. Natively `TELLEGEN_THREADS` and `TELLEGEN_RELAY_THREADS` win.
pub fn configure(spawner: Spawner, global: usize, relays: usize) {
    #[cfg(target_arch = "wasm32")]
    let _ = SPAWNER.set(spawner);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = spawner;
    let _ = COUNTS.set((global, relays));
    let _ = (SpinPool::global(), SpinPool::relays(), SpinPool::side());
}

/// The configured counts; natively the environment, then 3 and 2; in the
/// browser none without [`configure`].
fn counts() -> (usize, usize) {
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<usize>().ok());
    let set = COUNTS.get().copied();
    if cfg!(target_arch = "wasm32") {
        return set.unwrap_or((0, 0));
    }
    let (g, r) = set.unwrap_or((3, 2));
    (env("TELLEGEN_THREADS").unwrap_or(g), env("TELLEGEN_RELAY_THREADS").unwrap_or(r))
}

/// Start a thread running `f`; false where none can be started.
fn spawn(f: Box<dyn FnOnce() + Send>) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::Builder::new()
            .name("spin".into())
            .spawn(f)
            .expect("spin pool thread");
        true
    }
    #[cfg(target_arch = "wasm32")]
    match SPAWNER.get() {
        Some(s) => {
            s(f);
            true
        }
        None => false,
    }
}

struct Shared {
    /// The id of the newest job; a worker takes part in each new one.
    epoch: AtomicU64,
    /// The newest job's function (the two words of its reference) and task
    /// count, written before its id is published in `epoch`. The function is
    /// only called for tasks claimed under its own id (see `ticket`), and a
    /// job's words are only rewritten after all its tasks are done.
    job: [AtomicUsize; 2],
    tasks: AtomicUsize,
    /// `id << 32 | next task`: a task is claimed by raising it while the id
    /// is the job's own.
    ticket: AtomicU64,
    done: AtomicUsize,
    stop: AtomicBool,
}

pub(crate) struct SpinPool {
    shared: Arc<Shared>,
    threads: usize,
    /// One job at a time.
    busy: std::sync::Mutex<()>,
}

impl SpinPool {
    fn new(threads: usize) -> Self {
        let shared = Arc::new(Shared {
            epoch: AtomicU64::new(0),
            job: [AtomicUsize::new(0), AtomicUsize::new(0)],
            tasks: AtomicUsize::new(0),
            ticket: AtomicU64::new(0),
            done: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
        });
        let mut started = 0;
        for _ in 0..threads {
            let s = Arc::clone(&shared);
            if spawn(Box::new(move || worker(&s))) {
                started += 1;
            }
        }
        let threads = started;
        Self {
            shared,
            threads,
            busy: std::sync::Mutex::new(()),
        }
    }

    /// The process's pool.
    pub(crate) fn global() -> &'static SpinPool {
        static POOL: OnceLock<SpinPool> = OnceLock::new();
        POOL.get_or_init(|| SpinPool::new(counts().0))
    }

    /// The pool the relays are evaluated on (`TELLEGEN_RELAY_THREADS` besides the
    /// caller, default 2).
    pub(crate) fn relays() -> &'static SpinPool {
        static POOL: OnceLock<SpinPool> = OnceLock::new();
        POOL.get_or_init(|| SpinPool::new(counts().1))
    }

    /// The one thread the relays run on beside the step ([`Self::beside`]).
    pub(crate) fn side() -> &'static SpinPool {
        static POOL: OnceLock<SpinPool> = OnceLock::new();
        POOL.get_or_init(|| SpinPool::new(1))
    }

    pub(crate) fn threads(&self) -> usize {
        self.threads
    }

    /// Run `f(0..tasks)`, the caller taking part; returns when all are done.
    /// A nested or concurrent call runs its tasks on the caller alone.
    pub(crate) fn run<'a>(&self, tasks: usize, f: &'a (dyn Fn(usize) + Sync + 'a)) {
        if tasks == 0 {
            return;
        }
        let guard = match self.busy.try_lock() {
            Ok(g) if self.threads > 0 && tasks > 1 => g,
            _ => {
                (0..tasks).for_each(f);
                return;
            }
        };
        let id = self.publish(tasks, f);
        self.finish(id, tasks, f);
        drop(guard);
    }

    /// Run `side(0)` on a pool thread while the caller runs `main`, and
    /// return `main`'s result once both are done. Without a free thread the
    /// caller runs `side` first.
    pub(crate) fn beside<'a, R>(&self, side: &'a (dyn Fn(usize) + Sync + 'a), main: impl FnOnce() -> R) -> R {
        let guard = match self.busy.try_lock() {
            Ok(g) if self.threads > 0 => g,
            _ => {
                side(0);
                return main();
            }
        };
        let id = self.publish(1, side);
        let r = main();
        self.finish(id, 1, side);
        drop(guard);
        r
    }

    /// Publish a job of `tasks` tasks; its id.
    fn publish<'a>(&self, tasks: usize, f: &'a (dyn Fn(usize) + Sync + 'a)) -> u64 {
        let s = &*self.shared;
        // SAFETY: the job is called only for tasks claimed under this job's
        // id, all of which finish before `finish` returns.
        let job: &'static Job = unsafe { std::mem::transmute::<&'a (dyn Fn(usize) + Sync + 'a), &'static Job>(f) };
        let id = s.epoch.load(Ordering::Relaxed) + 1;
        // SAFETY: a shared reference to a trait object is two words.
        let words: [usize; 2] = unsafe { std::mem::transmute::<&'static Job, [usize; 2]>(job) };
        s.job[0].store(words[0], Ordering::Relaxed);
        s.job[1].store(words[1], Ordering::Relaxed);
        s.tasks.store(tasks, Ordering::Relaxed);
        s.done.store(0, Ordering::Relaxed);
        s.ticket.store(id << 32, Ordering::Release);
        s.epoch.store(id, Ordering::Release);
        id
    }

    /// Take part in job `id` until every task is done.
    fn finish(&self, id: u64, tasks: usize, f: &(dyn Fn(usize) + Sync)) {
        let s = &*self.shared;
        while let Some(k) = claim(s, id, tasks) {
            f(k);
            s.done.fetch_add(1, Ordering::AcqRel);
        }
        while s.done.load(Ordering::Acquire) < tasks {
            std::hint::spin_loop();
        }
    }
}

/// Claim the next task of job `id`, if any is left.
fn claim(s: &Shared, id: u64, tasks: usize) -> Option<usize> {
    let mut v = s.ticket.load(Ordering::Acquire);
    loop {
        if v >> 32 != id {
            return None;
        }
        let k = (v & 0xffff_ffff) as usize;
        if k >= tasks {
            return None;
        }
        match s.ticket.compare_exchange_weak(v, v + 1, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Some(k),
            Err(now) => v = now,
        }
    }
}

impl Drop for SpinPool {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn worker(s: &Shared) {
    let mut seen = s.epoch.load(Ordering::Acquire);
    let mut idle_since = Instant::now();
    let mut spins = 0u32;
    loop {
        if s.stop.load(Ordering::Relaxed) {
            return;
        }
        let e = s.epoch.load(Ordering::Acquire);
        if e == seen {
            spins += 1;
            if spins < spin_limit() {
                std::hint::spin_loop();
            } else if idle_since.elapsed() < Duration::from_millis(50) {
                // In the browser a yield does nothing: spin on.
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::yield_now();
                #[cfg(target_arch = "wasm32")]
                std::hint::spin_loop();
            } else {
                std::thread::sleep(Duration::from_micros(200));
            }
            continue;
        }
        seen = e;
        spins = 0;
        let words = [s.job[0].load(Ordering::Acquire), s.job[1].load(Ordering::Acquire)];
        let tasks = s.tasks.load(Ordering::Acquire);
        if words[0] != 0 && s.epoch.load(Ordering::Acquire) == e {
            // SAFETY: the words of job `e` (see `Shared::job`); it is only called
            // for a task claimed under `e`, which keeps the job alive.
            let f: &'static Job = unsafe { std::mem::transmute::<[usize; 2], &'static Job>(words) };
            while let Some(k) = claim(s, e, tasks) {
                f(k);
                s.done.fetch_add(1, Ordering::AcqRel);
            }
        }
        idle_since = Instant::now();
    }
}

/// `TELLEGEN_SPIN`: busy-wait rounds before an idle worker yields (default 20000).
fn spin_limit() -> u32 {
    static S: OnceLock<u32> = OnceLock::new();
    *S.get_or_init(|| std::env::var("TELLEGEN_SPIN").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000))
}
