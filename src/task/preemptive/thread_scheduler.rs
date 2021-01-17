use x86_64::VirtAddr;
use core::raw::TraitObject;
use core::mem;
use alloc::{boxed::Box, collections::{BTreeMap, BTreeSet, VecDeque}};
use super::{
    thread::{Thread, ThreadID},
    thread::SwitchReason
};
use crate::{print, println};

static SCHEDULER: spin::Mutex<Option<Scheduler>> = spin::Mutex::new(None);

#[derive(Debug)]
pub struct Scheduler
{
    threads: BTreeMap<ThreadID, Thread>,
    idle_thread_id: Option<ThreadID>,
    current_thread_id: ThreadID,
    paused_threads: VecDeque<ThreadID>,
    blocked_threads: BTreeSet<ThreadID>,
    wakeups: BTreeSet<ThreadID>,
}

impl Scheduler
{
    pub fn new() -> Self
    {
        let root_thread = Thread::create_root_thread();
        let root_id = root_thread.id();
        let mut threads = BTreeMap::new();
        threads
            .insert(root_id, root_thread)
            .expect_none("map is not empty after creation");

        Scheduler
        {
            threads,
            current_thread_id: root_id,
            paused_threads: VecDeque::new(),
            blocked_threads: BTreeSet::new(),
            wakeups: BTreeSet::new(),
            idle_thread_id: None,
        }
    }

    fn next_thread(&mut self) -> Option<ThreadID>
    {
        self.paused_threads.pop_front()
    }

    pub fn schedule(&mut self) -> Option<(VirtAddr, ThreadID)>
    {
        println!("{}", self.current_thread_id.as_u64());
        let mut next_thread_id = self.next_thread();
        if next_thread_id.is_none() && Some(self.current_thread_id) != self.idle_thread_id
        {
            next_thread_id = self.idle_thread_id;
        }

        if let Some(next_id) = next_thread_id
        {
            let next_thread = self
                .threads
                .get_mut(&next_id)
                .expect("next thread does not exist");

            let next_stack_pointer = next_thread
                .stack_pointer()
                .take()
                .expect("paused thread has no stack pointer");

            let prev_thread_id = mem::replace(&mut self.current_thread_id, next_thread.id());
            Some((next_stack_pointer, prev_thread_id))
        }
        else
        {
            None
        }
    }

    pub(super) fn add_paused_thread(&mut self, paused_stack_pointer: VirtAddr, paused_thread_id: ThreadID, switch_reason: SwitchReason)
    {
        let paused_thread = self
            .threads
            .get_mut(&paused_thread_id)
            .expect("paused thread does not exist");

        paused_thread
            .stack_pointer()
            .replace(paused_stack_pointer)
            .expect_none("running thread should have stack pointer set to None");

        if Some(paused_thread_id) == self.idle_thread_id
        {
            return;
        }

        match switch_reason
        {
            SwitchReason::Paused | SwitchReason::Yield =>
            {
                self.paused_threads.push_back(paused_thread_id);
            }
            SwitchReason::Blocked =>
            {
                self.blocked_threads.insert(paused_thread_id);
                self.check_for_wakeup(paused_thread_id);
            }
            SwitchReason::Exit =>
            {
                let _thread = self.threads.remove(&paused_thread_id).expect("thread not found");
                // TODO:
            }
        }
    }

    pub fn add_new_thread(&mut self, thread: Thread)
    {
        let thread_id = thread.id();
        self.threads
            .insert(thread_id, thread)
            .expect_none("thread already exists");
        self.paused_threads.push_back(thread_id);
    }

    pub fn set_idle_thread(&mut self, thread: Thread)
    {
        let thread_id = thread.id();
        self.threads
            .insert(thread_id, thread)
            .expect_none("thread already exists");
        self.idle_thread_id
            .replace(thread_id)
            .expect_none("idle thread should be set only once");
    }

    pub fn current_thread_id(&self) -> ThreadID
    {
        self.current_thread_id
    }

    fn check_for_wakeup(&mut self, thread_id: ThreadID)
    {
        if self.wakeups.remove(&thread_id)
        {
            assert!(self.blocked_threads.remove(&thread_id));
            self.paused_threads.push_back(thread_id);
        }
    }
}

pub fn invoke_scheduler()
{
    let next = SCHEDULER
        .try_lock()
        .and_then(|mut scheduler|
            scheduler.as_mut().and_then(|s| s.schedule()));

    if let Some((next_stack_pointer, prev_thread_id)) = next
    {
        unsafe 
        {
            super::context_switch::context_switch_to(next_stack_pointer, prev_thread_id, SwitchReason::Paused)
        };
    }
}

pub fn exit_thread() -> !
{
    synchronous_context_switch(SwitchReason::Exit).expect("can't exit last thread");
    unreachable!("finished thread continued");
}

pub fn yield_now()
{
    let _ = synchronous_context_switch(SwitchReason::Yield);
}

fn synchronous_context_switch(reason: SwitchReason) -> Result<(), ()>
{
    let next = with_scheduler(|s| s.schedule());

    match next
    {
        Some((next_stack_pointer, prev_thread_id)) =>
        unsafe {
            super::context_switch::context_switch_to(next_stack_pointer, prev_thread_id, reason);
            Ok(())
        }
        None => Err(())
    }
}

pub fn with_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&mut Scheduler) -> T
{
    f(SCHEDULER.lock().get_or_insert_with(Scheduler::new))
}

