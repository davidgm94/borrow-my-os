#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(osdev::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use osdev::println;
use osdev::print;
use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};
use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};
use osdev::task::cooperative::{Task, simple_executor::SimpleExecutor, executor::Executor};
use osdev::task::preemptive::thread::{Thread, };
use osdev::task::preemptive::thread_scheduler;

entry_point!(kernel_main);

//pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use osdev::memory::{self, BootInfoFrameAllocator};
    use osdev::allocator;
    use x86_64::{structures::paging::Page, VirtAddr};

    println!("Hello kernel{}", "!");
    osdev::init();

    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(physical_memory_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map)};

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    let idle_thread = Thread::create(idle_thread, 2, &mut mapper, &mut frame_allocator).unwrap();
    thread_scheduler::with_scheduler(|s| s.add_new_thread(idle_thread));

    for _ in 0..2
    {
        println!("Created thread");
        let thread = Thread::create(thread_entry, 2, &mut mapper, &mut frame_allocator).unwrap();
        thread_scheduler::with_scheduler(|s| s.add_new_thread(thread));
    }

    let thread = Thread::create_from_closure(|| thread_entry(), 2, &mut mapper, &mut frame_allocator).unwrap();
    thread_scheduler::with_scheduler(|s| s.add_new_thread(thread));


    let mut executor = Executor::new();
    executor.spawn(Task::new(osdev::task::cooperative::keyboard::print_keypresses()));
    executor.run();
    


    #[cfg(test)]
    test_main();

    println!("It didn't crash");
    osdev::hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    osdev::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    osdev::test_panic_handler(info)
}

async fn async_number() -> u32
{
    41
}

async fn example_task()
{
    let number = async_number().await;
    println!("async number: {}", number);
}

fn idle_thread() -> !
{
    loop
    {
        x86_64::instructions::hlt();
        thread_scheduler::yield_now();
    }
}

fn thread_entry() -> !
{
    let thread_id = thread_scheduler::with_scheduler(|s| s.current_thread_id().as_u64());
    if thread_id != 0
    {
        for _ in 0..=thread_id
        {
            println!("{}", thread_id);
            x86_64::instructions::hlt();
        }
    }
    thread_scheduler::exit_thread();
}
