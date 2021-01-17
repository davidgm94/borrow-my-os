#![no_std]
#![no_main]

use core::panic::PanicInfo;
use osdev::{QemuExitCode, exit_qemu, serial_println, serial_print};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    osdev::hlt_loop();
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    serial_println!("Serial print ran successfully!\n");
    exit_qemu(QemuExitCode::Success);
    osdev::hlt_loop();
}

