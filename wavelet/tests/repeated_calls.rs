use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static CASE_NUMBER: AtomicUsize = AtomicUsize::new(0);

fn compile_and_execute(source: &str, executions: &[(&str, &str)], options: &[&str]) {
    let directory = std::env::temp_dir().join(format!(
        "wavelet-repeated-calls-{}-{}",
        std::process::id(),
        CASE_NUMBER.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&directory).unwrap();
    let input = directory.join("source.rs");
    let output = directory.join("output.json");
    fs::write(&input, source).unwrap();
    let binary = env!("CARGO_BIN_EXE_wavelet");
    let compiled = Command::new(binary)
        .arg("compile")
        .arg(&input)
        .args(["--ghost-check", "z3", "-o"])
        .arg(&output)
        .args(options)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    for (arguments, expected) in executions {
        let executed = Command::new(binary)
            .arg("exec")
            .arg(&output)
            .args(["--args", arguments])
            .output()
            .unwrap();
        assert!(
            executed.status.success(),
            "execution failed:\n{}",
            String::from_utf8_lossy(&executed.stderr)
        );
        assert_eq!(
            String::from_utf8(executed.stdout).unwrap().trim(),
            *expected
        );
    }
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn repeated_scalar_calls_compile_and_execute() {
    compile_and_execute(
        r#"
        fn increment(n: i32) -> i32 {
            let one = 1;
            let result = n + one;
            result
        }
        fn twice(n: i32) -> i32 {
            let first = increment(n);
            let second = increment(first);
            second
        }
    "#,
        &[("5", "outputs: 7"), ("0", "outputs: 2")],
        &[],
    );
}

#[test]
fn parallel_calls_to_the_same_callee_execute_before_optimization() {
    // Isolate linking from the optimizer's separate parallel-call width bug.
    compile_and_execute(
        r#"
        fn increment(n: i32) -> i32 {
            let one = 1;
            let result = n + one;
            result
        }
        fn sum(n: i32) -> i32 {
            let first = increment(n);
            let second = increment(n);
            let result = first + second;
            result
        }
    "#,
        &[("5", "outputs: 12")],
        &["--no-opt"],
    );
}

#[test]
fn nested_repeated_calls_have_distinct_channels() {
    compile_and_execute(
        r#"
        fn increment(n: i32) -> i32 {
            let one = 1;
            let result = n + one;
            result
        }
        fn twice(n: i32) -> i32 {
            let first = increment(n);
            let second = increment(first);
            second
        }
        fn four_times(n: i32) -> i32 {
            let first = twice(n);
            let second = twice(first);
            second
        }
    "#,
        &[("5", "outputs: 9")],
        &[],
    );
}

#[test]
fn disjoint_array_calls_compile_without_a_fence() {
    compile_and_execute(
        r#"
        #[cap(a: uniq @ n + 1..n + 2)]
        fn write_next(n: i32, value: i32, a: &mut [i32; 16]) {
            let one = 1;
            let next = n + one;
            a[next] = value;
        }
        #[cap(a: uniq @ 1..3)]
        fn wrapper(value: i32, a: &mut [i32; 16]) {
            let zero = 0;
            let one = 1;
            let first = write_next(zero, value, a);
            let second = write_next(one, value, a);
        }
    "#,
        &[],
        &[],
    );
}
