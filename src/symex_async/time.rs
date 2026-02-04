use super::executor::with_current_executor;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub fn now_ms() -> u64 {
    with_current_executor(|ex| ex.map(|e| e.now_ms()).unwrap_or(0))
}

pub fn sleep(dur: std::time::Duration) -> Sleep {
    let ms = dur.as_millis() as u64;
    let deadline = now_ms().saturating_add(ms);
    Sleep {
        deadline_ms: deadline,
        registered: false,
    }
}

pub struct Sleep {
    deadline_ms: u64,
    registered: bool,
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = now_ms();
        if now >= self.deadline_ms {
            return Poll::Ready(());
        }

        if !self.registered {
            with_current_executor(|ex| {
                let ex = ex.expect("symex_async::time::sleep used outside symex_async::run");
                ex.register_timer(self.deadline_ms, cx.waker().clone());
            });
            self.registered = true;
        }

        Poll::Pending
    }
}
