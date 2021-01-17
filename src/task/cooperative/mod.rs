pub mod simple_executor;
pub mod executor;
pub mod keyboard;

use core::{task::{Context, Poll}, future::Future, pin::Pin, sync::atomic::{AtomicU64, Ordering}};
use alloc::boxed::Box;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskID(u64);

impl TaskID
{
    fn new() -> Self
    {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskID(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
} 

pub struct Task
{
    id: TaskID,
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task
{
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task
    {
        Task
        {
            id: TaskID::new(),
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()>
    {
        self.future.as_mut().poll(context)
    }
}
