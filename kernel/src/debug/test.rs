#[allow(unconditional_recursion)]
#[allow(unused)]
fn stack_overflow() {
    stack_overflow();
    stack_overflow();
}

#[allow(unused)]
fn page_fault() {
    unsafe {
        *(0xdeadbeef as *mut u8) = 42;
    }
}

fn binary_test_test() {
    assert!(crate::utils::binary_test(0b001000u64, 3));
    assert!(!crate::utils::binary_test(0b010000u64, 3));
}

pub fn run_tests(tests: &[&dyn Fn()]) {
    use crate::println;
    println!("Running {} tests", tests.len());
    for (index, test) in tests.iter().enumerate() {
        test();
        println!("Test {} succeeded!", index + 1);
    }
}

pub fn test_main() {
    run_tests(&[&binary_test_test]);
}
