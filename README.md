# The 4th gen of Dvida OS powered with rust and limine

Hi there, this is a mini operating system that I am developing

for testing:
for each test case, use:
```rust
#[test_case]
#[allow(unreachable_code)]
fn name() {
    // add this to ignore it
    ignore!();
    test_name!("foo");
    end_test!();
}

```

Current WIP:
Storage

Future WIPs:
syscalls
schedulers
memory management
a homebrew heap allocator
...
