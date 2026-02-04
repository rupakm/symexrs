use crate::runtime;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub type TaskId = usize;

#[derive(Clone)]
pub struct Executor {
    inner: Arc<Mutex<ExecutorInner>>,
}

struct Task {
    future: Pin<Box<dyn Future<Output = ()> + 'static>>,
    queued: bool,
    completed: bool,
}

struct ExecutorInner {
    tasks: Vec<Option<Task>>,
    ready: VecDeque<TaskId>,
    // Virtual time support
    now_ms: u64,
    timers: std::collections::BinaryHeap<std::cmp::Reverse<TimerEntry>>,
    timer_seq: u64,
}

#[derive(Clone)]
struct TimerEntry {
    when_ms: u64,
    seq: u64,
    waker: Waker,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.when_ms == other.when_ms && self.seq == other.seq
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.when_ms.cmp(&other.when_ms) {
            std::cmp::Ordering::Equal => self.seq.cmp(&other.seq),
            o => o,
        }
    }
}

impl Executor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ExecutorInner {
                tasks: Vec::new(),
                ready: VecDeque::new(),
                now_ms: 0,
                timers: std::collections::BinaryHeap::new(),
                timer_seq: 0,
            })),
        }
    }

    pub fn now_ms(&self) -> u64 {
        self.inner.lock().unwrap().now_ms
    }

    pub fn register_timer(&self, when_ms: u64, waker: Waker) {
        let mut inner = self.inner.lock().unwrap();
        let seq = inner.timer_seq;
        inner.timer_seq = inner.timer_seq.wrapping_add(1);
        inner.timers.push(std::cmp::Reverse(TimerEntry { when_ms, seq, waker }));
    }

    pub fn spawn<Fut, T>(&self, fut: Fut) -> JoinHandle<T>
    where
        Fut: Future<Output = T> + 'static,
        T: 'static,
    {
        let state: Arc<Mutex<JoinState<T>>> = Arc::new(Mutex::new(JoinState { result: None, waker: None }));
        let state2 = Arc::clone(&state);
        let task_fut = async move {
            let out = fut.await;
            let mut st = state2.lock().unwrap();
            st.result = Some(out);
            if let Some(w) = st.waker.take() {
                w.wake();
            }
        };

        let id = {
            let mut inner = self.inner.lock().unwrap();
            let id = inner.tasks.len();
            inner.tasks.push(Some(Task {
                future: Box::pin(task_fut),
                queued: true,
                completed: false,
            }));
            inner.ready.push_back(id);
            id
        };

        // Ensure wakers can re-enqueue.
        let _ = id;

        JoinHandle { state }
    }

    fn poll_task(&self, id: TaskId) {
        let waker = task_waker(Arc::clone(&self.inner), id);
        let mut cx = Context::from_waker(&waker);

        // Poll outside the lock to avoid re-entrancy deadlocks.
        let mut fut: Option<Pin<Box<dyn Future<Output = ()> + 'static>>> = None;
        {
            let mut inner = self.inner.lock().unwrap();
            if let Some(task) = inner.tasks.get_mut(id).and_then(|t| t.as_mut()) {
                task.queued = false;
                if task.completed {
                    return;
                }
                // Temporarily move the future out.
                let placeholder: Pin<Box<dyn Future<Output = ()> + 'static>> = Box::pin(async {});
                let f = std::mem::replace(&mut task.future, placeholder);
                fut = Some(f);
            }
        }

        if let Some(mut f) = fut {
            let polled = f.as_mut().poll(&mut cx);
            let mut inner = self.inner.lock().unwrap();
            if let Some(task) = inner.tasks.get_mut(id).and_then(|t| t.as_mut()) {
                match polled {
                    Poll::Ready(()) => {
                        task.completed = true;
                        task.future = Box::pin(async {});
                    }
                    Poll::Pending => {
                        task.future = f;
                    }
                }
            }
        }
    }

    fn has_live_tasks(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner
            .tasks
            .iter()
            .any(|t| t.as_ref().map(|t| !t.completed).unwrap_or(false))
    }

    fn pop_ready(&self) -> Option<TaskId> {
        let mut inner = self.inner.lock().unwrap();
        inner.ready.pop_front()
    }

    fn ready_len(&self) -> usize {
        self.inner.lock().unwrap().ready.len()
    }

    fn choose_ready_task(&self) -> Option<TaskId> {
        let len = self.ready_len();
        if len == 0 {
            return None;
        }
        if len == 1 {
            return self.pop_ready();
        }

        // Build stable candidate list from current ready queue (preserve order, dedup).
        let candidates = {
            let inner = self.inner.lock().unwrap();
            let mut out: Vec<TaskId> = Vec::new();
            let mut seen: std::collections::HashSet<TaskId> = std::collections::HashSet::new();
            for id in inner.ready.iter().copied() {
                if seen.insert(id) {
                    out.push(id);
                }
            }
            out
        };

        let arity = candidates.len() as u32;
        let chosen = runtime::choose_choice(arity, 0);
        let chosen_id = candidates[chosen as usize];

        // Remove first occurrence from ready queue.
        {
            let mut inner = self.inner.lock().unwrap();
            if let Some(pos) = inner.ready.iter().position(|x| *x == chosen_id) {
                inner.ready.remove(pos);
            }
        }
        Some(chosen_id)
    }

    fn advance_time_if_needed(&self) {
        let mut to_wake: Vec<Waker> = Vec::new();
        let new_now = {
            let mut inner = self.inner.lock().unwrap();
            let first = match inner.timers.pop() {
                Some(std::cmp::Reverse(t)) => t,
                None => return,
            };

            let when = first.when_ms;
            inner.now_ms = when;
            to_wake.push(first.waker);

            while let Some(std::cmp::Reverse(next)) = inner.timers.peek().cloned() {
                if next.when_ms != when {
                    break;
                }
                let std::cmp::Reverse(t) = inner.timers.pop().unwrap();
                to_wake.push(t.waker);
            }
            when
        };

        let _ = new_now;
        for w in to_wake {
            w.wake();
        }
    }

    pub fn run_until_stalled(&self) {
        loop {
            if !self.has_live_tasks() {
                return;
            }

            let Some(id) = self.choose_ready_task() else {
                // No runnable tasks; try to advance time.
                self.advance_time_if_needed();
                if self.ready_len() == 0 {
                    panic!("deadlock: no runnable tasks and no pending timers");
                }
                continue;
            };

            self.poll_task(id);
        }
    }
}

struct JoinState<T> {
    result: Option<T>,
    waker: Option<Waker>,
}

pub struct JoinHandle<T> {
    state: Arc<Mutex<JoinState<T>>>,
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = self.state.lock().unwrap();
        if let Some(v) = st.result.take() {
            Poll::Ready(v)
        } else {
            st.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

thread_local! {
    static CURRENT_EXECUTOR: RefCell<Option<Executor>> = const { RefCell::new(None) };
}

pub fn with_current_executor<R>(f: impl FnOnce(Option<&Executor>) -> R) -> R {
    CURRENT_EXECUTOR.with(|c| f(c.borrow().as_ref()))
}

pub fn set_current_executor(ex: Option<Executor>) {
    CURRENT_EXECUTOR.with(|c| *c.borrow_mut() = ex);
}

pub fn spawn<Fut, T>(fut: Fut) -> JoinHandle<T>
where
    Fut: Future<Output = T> + 'static,
    T: 'static,
{
    with_current_executor(|ex| {
        let ex = ex.expect("symex_async::spawn called outside symex_async::run");
        ex.spawn(fut)
    })
}

pub fn run<Fut>(fut: Fut) -> Fut::Output
where
    Fut: Future + 'static,
    Fut::Output: 'static,
{
    let ex = Executor::new();
    set_current_executor(Some(ex.clone()));

    let out: Arc<Mutex<Option<Fut::Output>>> = Arc::new(Mutex::new(None));
    let out2 = Arc::clone(&out);

    // Spawn the main future as a task.
    let _main = ex.spawn(async move {
        let v = fut.await;
        *out2.lock().unwrap() = Some(v);
    });

    ex.run_until_stalled();
    set_current_executor(None);

    out.lock()
        .unwrap()
        .take()
        .expect("symex_async::run: main future did not complete")
}

pub struct YieldNow {
    yielded: bool,
}

pub fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

struct TaskWakerData {
    exec: Arc<Mutex<ExecutorInner>>,
    id: TaskId,
}

fn task_waker(exec: Arc<Mutex<ExecutorInner>>, id: TaskId) -> Waker {
    let data = Arc::new(TaskWakerData { exec, id });
    unsafe { Waker::from_raw(raw_waker(Arc::into_raw(data) as *const ())) }
}

unsafe fn raw_waker(ptr: *const ()) -> RawWaker {
    RawWaker::new(ptr, &VTABLE)
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker);

unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
    let arc = unsafe { Arc::<TaskWakerData>::from_raw(ptr as *const TaskWakerData) };
    let cloned = Arc::clone(&arc);
    std::mem::forget(arc);
    RawWaker::new(Arc::into_raw(cloned) as *const (), &VTABLE)
}

unsafe fn wake(ptr: *const ()) {
    let arc = unsafe { Arc::<TaskWakerData>::from_raw(ptr as *const TaskWakerData) };
    enqueue_task(&arc.exec, arc.id);
}

unsafe fn wake_by_ref(ptr: *const ()) {
    let arc = unsafe { Arc::<TaskWakerData>::from_raw(ptr as *const TaskWakerData) };
    enqueue_task(&arc.exec, arc.id);
    std::mem::forget(arc);
}

unsafe fn drop_waker(ptr: *const ()) {
    drop(unsafe { Arc::<TaskWakerData>::from_raw(ptr as *const TaskWakerData) });
}

fn enqueue_task(exec: &Arc<Mutex<ExecutorInner>>, id: TaskId) {
    let mut inner = exec.lock().unwrap();
    if let Some(task) = inner.tasks.get_mut(id).and_then(|t| t.as_mut()) {
        if task.completed || task.queued {
            return;
        }
        task.queued = true;
        inner.ready.push_back(id);
    }
}
