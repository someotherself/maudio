# Maudio testing

```text
This file contains author notes about running the test suite for maudio
```

## Normal testing of the crate

The feature "ci-tests" exists mostly for Github actions. It will disable the backend to avoid errors.
All tests when initializing an `Engine`, `Context` or `Device` must use the `new_for_tests()` function.
Otherwise, normal testing can be done without any feature enabled.

## Testing the ffi interface with valgrind

First create a binary with the tests without running them
```bash
cargo test --no-run
```
This will create a binary that we can use. In the case below it is `target/debug/deps/maudio-386a551b8171a857`
```text
   Compiling maudio-sys v0.1.0 (/home/cristian/Rust/projects/maudio/crates/maudio-sys)
   Compiling maudio v0.1.0 (/home/cristian/Rust/projects/maudio/crates/maudio)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.00s
  Executable unittests src/lib.rs (target/debug/deps/maudio-386a551b8171a857) <--
  Executable unittests src/lib.rs (target/debug/deps/maudio_sys-a42920ded4aaa31b)
```

We can then use valgrind to test the memory safety of the app. The recommended command is:

A single test:

You can list all the tests available using:
```
target/debug/deps/maudio-386a551b8171a857 --list
```

```bash
valgrind -s --leak-check=full --show-leak-kinds=definite,indirect,possible \
  --errors-for-leak-kinds=definite,indirect \
  --track-origins=yes \
  target/debug/deps/maudio-386a551b8171a857 engine::node_graph::nodes::effects::delay::test::test_delay_node_test_set_get_wet_roundtrip --exact --nocapture --test-threads=1
```


All tests:
```bash
valgrind -s --leak-check=full --show-leak-kinds=all --track-origins=yes \
  target/debug/deps/maudio-386a551b8171a857 \
  --nocapture \
  --test-threads=1
```

When runinng cargo test, there may still be an error that show like:
```
possibly lost: 48 bytes in 1 blocks
```

This is usually the cargo test harness itself and it not an actual memory leak.
It can be traced by using the flag:
```
--show-leak-kinds=possible
```
