#[cfg(test)]
#[macro_export]
macro_rules! ignore {
    () => {
        return;
    };
}

#[cfg(test)]
#[macro_export]
macro_rules! test_name {
    ($name: expr) => {
        $crate::println!("running test: {}", $name);
    };
}

#[cfg(test)]
#[macro_export]
macro_rules! end_test {
    () => {
        $crate::println!("test succeeded!");
    };
}

#[test_case]
#[allow(unreachable_code)]
fn page_fault() {
    ignore!();
    test_name!("page fault");

    unsafe {
        *(0xdeadbeef as *mut u8) = 42;
    }

    end_test!();
}

#[cfg(test)]
pub fn run_tests(tests: &[&dyn Fn()]) {
    use crate::println;
    println!("Found {} tests in total", tests.len());
    for (_index, test) in tests.iter().enumerate() {
        test();
    }
}
