use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Debug)]
pub struct Canceled;

struct Inner<T> {
    value: Option<T>,
    closed: bool,
    recv_waker: Option<Waker>,
}

pub struct Sender<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

pub struct Receiver<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Mutex::new(Inner {
        value: None,
        closed: false,
        recv_waker: None,
    }));
    (
        Sender {
            inner: Arc::clone(&inner),
        },
        Receiver { inner },
    )
}

impl<T> Sender<T> {
    pub fn send(self, value: T) -> Result<(), T> {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed || inner.value.is_some() {
            return Err(value);
        }
        inner.value = Some(value);
        if let Some(w) = inner.recv_waker.take() {
            w.wake();
        }
        Ok(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed {
            return;
        }
        inner.closed = true;
        if let Some(w) = inner.recv_waker.take() {
            w.wake();
        }
    }
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, Canceled>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(v) = inner.value.take() {
            inner.closed = true;
            return Poll::Ready(Ok(v));
        }
        if inner.closed {
            return Poll::Ready(Err(Canceled));
        }
        inner.recv_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}
