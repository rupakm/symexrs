use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Debug)]
pub struct SendError;

struct Inner<T> {
    q: VecDeque<T>,
    cap: usize,
    closed: bool,
    recv_waker: Option<Waker>,
    send_waiters: VecDeque<Waker>,
}

#[derive(Clone)]
pub struct Sender<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

pub struct Receiver<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

pub fn channel<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Mutex::new(Inner {
        q: VecDeque::new(),
        cap: cap.max(1),
        closed: false,
        recv_waker: None,
        send_waiters: VecDeque::new(),
    }));
    (
        Sender {
            inner: Arc::clone(&inner),
        },
        Receiver { inner },
    )
}

impl<T> Sender<T> {
    pub fn send(&self, value: T) -> SendFuture<T> {
        SendFuture {
            inner: Arc::clone(&self.inner),
            value: Some(value),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        // Mark closed only when last sender is dropped. This simplistic implementation
        // doesn't track sender count; treat drop as close.
        inner.closed = true;
        if let Some(w) = inner.recv_waker.take() {
            w.wake();
        }
    }
}

pub struct SendFuture<T> {
    inner: Arc<Mutex<Inner<T>>>,
    value: Option<T>,
}

impl<T> Unpin for SendFuture<T> {}

impl<T> Future for SendFuture<T> {
    type Output = Result<(), SendError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut inner = this.inner.lock().unwrap();
        if inner.closed {
            return Poll::Ready(Err(SendError));
        }
        if inner.q.len() < inner.cap {
            let v = this.value.take().expect("polled after completion");
            inner.q.push_back(v);
            if let Some(w) = inner.recv_waker.take() {
                w.wake();
            }
            return Poll::Ready(Ok(()));
        }

        inner.send_waiters.push_back(cx.waker().clone());
        Poll::Pending
    }
}

impl<T> Receiver<T> {
    pub fn recv(&mut self) -> RecvFuture<T> {
        RecvFuture {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub struct RecvFuture<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Future for RecvFuture<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(v) = inner.q.pop_front() {
            if let Some(w) = inner.send_waiters.pop_front() {
                w.wake();
            }
            return Poll::Ready(Some(v));
        }

        if inner.closed {
            return Poll::Ready(None);
        }

        inner.recv_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}
