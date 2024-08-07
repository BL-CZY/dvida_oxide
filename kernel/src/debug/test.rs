#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow();
    stack_overflow();
}

fn page_fault() {
    unsafe {
        *(0xdeadbeef as *mut u8) = 42;
    }
}

pub fn run_tests(tests: &[&dyn Fn()]) {
    use crate::println;
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}

pub fn test_main() {
    run_tests(&[&page_fault, &stack_overflow]);
}
