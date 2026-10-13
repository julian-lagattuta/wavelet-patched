//! Tests using the parser to check annotated Rust programs

use std::collections::HashMap;
use wavelet_elab::check::{check_fn_with_options, CheckOptions};
use wavelet_elab::env::FnRegistry;
use wavelet_elab::ir::{FnDef, Signedness, Stmt, Tail, Ty};
use wavelet_elab::logic::syntactic::SyntacticLogic;
use wavelet_elab::SemanticLogic;
use wavelet_elab::{infer_program, parse_program, UntypedVar};

// Define the fence macro as a no-op for parsing
#[allow(unused_macros)]
macro_rules! fence {
    ($($tt:tt)*) => {};
}

fn parse_fixture(code: &str) -> HashMap<String, FnDef<UntypedVar>> {
    let program = parse_program(code).expect("Failed to parse fixture file");
    program
        .defs
        .into_iter()
        .map(|def| {
            let key = def.name.clone().0;
            (key, def)
        })
        .collect()
}

fn parse_fn_by_name(code: &str, fn_name: &str) -> FnDef<UntypedVar> {
    let program =
        parse_program(code).unwrap_or_else(|err| panic!("Failed to parse program: {:?}", err));
    program
        .defs
        .into_iter()
        .find(|def| def.name.0 == fn_name)
        .unwrap_or_else(|| panic!("Missing function `{}` in input", fn_name))
}

macro_rules! with_backends {
    (($name:ident, $logic:ident) => $body:block) => {{
        let $name = "semantic";
        let semantic_logic = SemanticLogic::default();
        let $logic = &semantic_logic;
        $body
    }
    {
        let $name = "syntactic";
        let syntactic_logic = SyntacticLogic::default();
        let $logic = &syntactic_logic;
        $body
    }};
}

macro_rules! single_fn_parser_test_ok {
    ($test_name:ident, $file:expr, $fn_name:expr, $opts:expr) => {
        #[test]
        fn $test_name() {
            let code = include_str!($file);
            let fn_def = parse_fn_by_name(code, $fn_name);
            let mut registry = FnRegistry::default();
            registry.insert(fn_def.clone());
            let options: CheckOptions = $opts;
            with_backends!((name, logic) => {
                let result = check_fn_with_options(&fn_def, &registry, logic, options);
                assert!(result.is_ok(), "{name} backend failed: {:?}", result.err());
            });
        }
    };
}

macro_rules! single_fn_parser_test_err {
    ($test_name:ident, $file:expr, $fn_name:expr, $opts:expr, $debug:expr) => {
        #[test]
        fn $test_name() {
            let code = include_str!($file);
            let fn_def = parse_fn_by_name(code, $fn_name);
            let mut registry = FnRegistry::default();
            registry.insert(fn_def.clone());
            let options: CheckOptions = $opts;
            with_backends!((name, logic) => {
                let result = check_fn_with_options(&fn_def, &registry, logic, options);
                if $debug {
                    println!("Result ({name}): {:?}", result);
                }
                assert!(
                    result.is_err(),
                    "Expected type checking to fail ({name}), got {:?}",
                    result
                );
            });
        }
    };
}

macro_rules! parser_case {
    ($test_name:ident, file = $file:expr, fn = $fn_name:expr, expect = ok, options = $opts:expr $(,)?) => {
        single_fn_parser_test_ok!($test_name, $file, $fn_name, $opts);
    };
    ($test_name:ident, file = $file:expr, fn = $fn_name:expr, expect = ok $(,)?) => {
        parser_case!(
            $test_name,
            file = $file,
            fn = $fn_name,
            expect = ok,
            options = CheckOptions::default()
        );
    };
    ($test_name:ident, file = $file:expr, fn = $fn_name:expr, expect = err, options = $opts:expr, debug = $debug:expr $(,)?) => {
        single_fn_parser_test_err!($test_name, $file, $fn_name, $opts, $debug);
    };
    ($test_name:ident, file = $file:expr, fn = $fn_name:expr, expect = err, options = $opts:expr $(,)?) => {
        parser_case!(
            $test_name,
            file = $file,
            fn = $fn_name,
            expect = err,
            options = $opts,
            debug = false
        );
    };
    ($test_name:ident, file = $file:expr, fn = $fn_name:expr, expect = err, debug = $debug:expr $(,)?) => {
        parser_case!(
            $test_name,
            file = $file,
            fn = $fn_name,
            expect = err,
            options = CheckOptions::default(),
            debug = $debug
        );
    };
    ($test_name:ident, file = $file:expr, fn = $fn_name:expr, expect = err $(,)?) => {
        parser_case!(
            $test_name,
            file = $file,
            fn = $fn_name,
            expect = err,
            options = CheckOptions::default(),
            debug = false
        );
    };
}

macro_rules! fixture_parser_test {
    ($test_name:ident, file = $file:expr, entry = $entry:expr, extra = [$($extra:expr),* $(,)?], options = $opts:expr, backends = [$($backend:expr),+ $(,)?] $(,)?) => {
        #[test]
        fn $test_name() {
            let defs = parse_fixture(include_str!($file));
            let mut registry = FnRegistry::default();
            let top_def = defs
                .get($entry)
                .unwrap_or_else(|| panic!("Missing {} in fixture", $entry))
                .clone();
            registry.insert(top_def.clone());
            $(
                let def = defs
                    .get($extra)
                    .unwrap_or_else(|| panic!("Missing {} in fixture", $extra))
                    .clone();
                registry.insert(def);
            )*
            let options: CheckOptions = $opts;
            let allowed_backends = [$( $backend ),+];
            with_backends!((name, logic) => {
                if allowed_backends.contains(&name) {
                    // Check extra functions
                    $(
                        let extra_def = defs.get($extra).unwrap();
                        let extra_result = check_fn_with_options(extra_def, &registry, logic, options);
                        assert!(extra_result.is_ok(), "{name} backend failed for {}: {:?}", $extra, extra_result.err());
                    )*
                    // Check entry function
                    let result = check_fn_with_options(&top_def, &registry, logic, options);
                    assert!(result.is_ok(), "{name} backend failed: {:?}", result.err());
                }
            });
        }
    };
    ($test_name:ident, file = $file:expr, entry = $entry:expr, extra = [$($extra:expr),* $(,)?], backends = [$($backend:expr),+ $(,)?] $(,)?) => {
        fixture_parser_test!(
            $test_name,
            file = $file,
            entry = $entry,
            extra = [$($extra),*],
            options = CheckOptions::default(),
            backends = [$($backend),+],
        );
    };
    ($test_name:ident, file = $file:expr, entry = $entry:expr, extra = [$($extra:expr),* $(,)?], options = $opts:expr $(,)?) => {
        fixture_parser_test!(
            $test_name,
            file = $file,
            entry = $entry,
            extra = [$($extra),*],
            options = $opts,
            backends = ["semantic", "syntactic"],
        );
    };
    ($test_name:ident, file = $file:expr, entry = $entry:expr, extra = [$($extra:expr),* $(,)?]) => {
        fixture_parser_test!(
            $test_name,
            file = $file,
            entry = $entry,
            extra = [$($extra),*],
            options = CheckOptions::default(),
            backends = ["semantic", "syntactic"],
        );
    };
}

parser_case!(
    test_sum,
    file = "test_files/sum.rs",
    fn = "sum",
    expect = ok
);

parser_case!(
    test_zero_out,
    file = "test_files/zero_out.rs",
    fn = "zero_out",
    expect = ok,
    options = CheckOptions {
        verbose: true,
        ..Default::default()
    }
);

parser_case!(
    test_copy_array,
    file = "test_files/copy_array.rs",
    fn = "copy_array",
    expect = ok,
    options = CheckOptions {
        verbose: true,
        ..Default::default()
    }
);

parser_case!(
    test_increment,
    file = "test_files/increment.rs",
    fn = "increment",
    expect = ok,
    options = CheckOptions {
        verbose: true,
        ..Default::default()
    }
);

parser_case!(
    test_increment_without_fence_fails,
    file = "test_files/increment_bad.rs",
    fn = "increment_bad",
    expect = err,
    debug = true
);

parser_case!(
    test_raw_with_fence,
    file = "test_files/raw.rs",
    fn = "raw",
    expect = ok,
    options = CheckOptions {
        verbose: true,
        log_solver_queries: false,
    }
);

parser_case!(
    test_raw_without_fence_fails,
    file = "test_files/raw_no_fence.rs",
    fn = "raw_bad",
    expect = err,
    options = CheckOptions {
        verbose: true,
        ..Default::default()
    }
);

parser_case!(
    test_war_with_fence,
    file = "test_files/war.rs",
    fn = "war",
    expect = ok
);

parser_case!(
    test_war_without_fence_fails,
    file = "test_files/war_no_fence.rs",
    fn = "war_bad",
    expect = err,
    options = CheckOptions {
        verbose: true,
        ..Default::default()
    }
);

parser_case!(
    test_waw_with_fence,
    file = "test_files/waw.rs",
    fn = "waw",
    expect = ok
);

parser_case!(
    test_waw_without_fence_fails,
    file = "test_files/waw_no_fence.rs",
    fn = "waw_bad",
    expect = err,
    options = CheckOptions {
        verbose: true,
        ..Default::default()
    }
);

fixture_parser_test!(
    test_nn_relu_with_parser,
    file = "test_files/nn_relu.rs",
    entry = "nn_relu",
    extra = ["nn_relu_aux"]
);

fixture_parser_test!(
    test_nn_fc_with_parser,
    file = "test_files/nn_fc.rs",
    entry = "nn_fc",
    extra = ["row_dot", "rec_rows", "clamp_i16"],
    options = CheckOptions {
        verbose: false,
        log_solver_queries: false,
    },
    backends = ["semantic"]
);

fixture_parser_test!(
    test_dmv_with_parser,
    file = "test_files/dmv.rs",
    entry = "dmv",
    extra = ["cal_dot_product", "mv_mul"],
    options = CheckOptions {
        verbose: false,
        log_solver_queries: false,
    },
    backends = ["semantic"],
);

parser_case!(
    test_split_region_write,
    file = "test_files/split_region.rs",
    fn = "split_write",
    expect = ok
);

parser_case!(
    test_split_region_hole_fails,
    file = "test_files/split_region.rs",
    fn = "split_write_hole",
    expect = err
);

parser_case!(
    test_repeated_annotation_is_unioned,
    file = "test_files/split_region.rs",
    fn = "repeated_annotation",
    expect = ok
);

parser_case!(
    test_mixed_permissions_on_one_array,
    file = "test_files/split_region.rs",
    fn = "mixed_permissions",
    expect = ok
);

parser_case!(
    test_singleton_and_empty_region,
    file = "test_files/split_region.rs",
    fn = "singleton_region",
    expect = ok
);

fixture_parser_test!(
    test_caps_matched_to_arrays_by_name,
    file = "test_files/reordered_caps.rs",
    entry = "copy_entry",
    extra = ["copy_all"]
);

#[test]
fn positive_literal_infers_signed_return_type() {
    let def = parse_fn_by_name("fn inferred() -> i32 { let zero = 0; zero }", "inferred");
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let typed = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    )
    .unwrap();
    let Stmt::LetVal { var, .. } = &typed.body.stmts[0] else {
        panic!("expected literal binding");
    };
    assert_eq!(var.ty, Ty::Int(Signedness::Signed));
}

#[test]
fn public_infer_program_does_not_check_capabilities() {
    let program =
        parse_program("fn load(A: &[i32; 1]) -> i32 { let value = A[0]; value }").unwrap();
    let typed = infer_program(&program).unwrap();
    let Stmt::LetOp { vars, .. } = &typed.defs[0].body.stmts[1] else {
        panic!("expected load statement");
    };
    assert_eq!(vars[0].ty, Ty::Int(Signedness::Signed));
}

#[test]
fn public_infer_program_accepts_interleaved_array_parameters() {
    let mut program = parse_program(
        "fn callee(x: i32, y: i32, A: &[i32; 1]) -> i32 { x }\
         fn caller(x: i32, y: i32, A: &[i32; 1]) -> i32 { callee(x, y, A) }",
    )
    .unwrap();

    program.defs[0].params.swap(1, 2);
    let Tail::TailCall { args, .. } = &mut program.defs[1].body.tail else {
        panic!("expected tail call");
    };
    args.swap(1, 2);

    infer_program(&program).unwrap();
}

#[test]
fn unconstrained_positive_literal_defaults_unsigned() {
    let def = parse_fn_by_name("fn inferred() -> u32 { let zero = 0; zero }", "inferred");
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let typed = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    )
    .unwrap();
    let Stmt::LetVal { var, .. } = &typed.body.stmts[0] else {
        panic!("expected literal binding");
    };
    assert_eq!(var.ty, Ty::Int(Signedness::Unsigned));
}

#[test]
fn call_context_types_literal() {
    let defs = parse_fixture("fn identity(x: i32) -> i32 { x } fn caller() -> i32 { identity(0) }");
    let caller = defs["caller"].clone();
    let mut registry = FnRegistry::default();
    for def in defs.into_values() {
        registry.insert(def);
    }
    let typed = check_fn_with_options(
        &caller,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    )
    .unwrap();
    let Stmt::LetVal { var, .. } = &typed.body.stmts[0] else {
        panic!("expected parser-generated literal binding");
    };
    assert_eq!(var.ty, Ty::Int(Signedness::Signed));
}

#[test]
fn branch_context_types_both_literals() {
    let def = parse_fn_by_name(
        "fn choose(c: bool) -> i32 { if c { 0 } else { 1 } }",
        "choose",
    );
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let typed = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    )
    .unwrap();
    let wavelet_elab::Tail::IfElse { then_e, else_e, .. } = &typed.body.tail else {
        panic!("expected conditional tail");
    };
    for branch in [then_e, else_e] {
        let Stmt::LetVal { var, .. } = &branch.stmts[0] else {
            panic!("expected literal binding");
        };
        assert_eq!(var.ty, Ty::Int(Signedness::Signed));
    }
}

#[test]
fn arithmetic_unifies_operand_and_result_types() {
    let def = parse_fn_by_name(
        "fn add() -> i32 { let x = 0; let y = 1; let z = x + y; z }",
        "add",
    );
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let typed = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    )
    .unwrap();
    let Stmt::LetOp { vars, .. } = &typed.body.stmts[2] else {
        panic!("expected addition");
    };
    assert!(vars.iter().all(|var| var.ty == Ty::Int(Signedness::Signed)));
}

#[test]
fn negative_literal_cannot_be_unsigned() {
    let def = parse_fn_by_name("fn invalid() -> u32 { let n = -1; n }", "invalid");
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let result = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    );
    assert!(matches!(
        result,
        Err(wavelet_elab::TypeError::TypeMismatch { .. })
    ));
}

#[test]
fn mixed_signedness_operation_is_rejected() {
    let def = parse_fn_by_name(
        "fn invalid(x: i32, y: u32) -> i32 { let z = x + y; z }",
        "invalid",
    );
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let result = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    );
    assert!(matches!(
        result,
        Err(wavelet_elab::TypeError::TypeMismatch { .. })
    ));
}

#[test]
fn conflicting_literal_contexts_are_rejected() {
    let defs = parse_fixture(
        "
        fn signed(x: i32) -> i32 { x }
        fn unsigned(x: u32) -> u32 { x }
        fn conflict() -> u32 {
            let x = 0;
            let ignored = signed(x);
            unsigned(x)
        }
        ",
    );
    let conflict = defs["conflict"].clone();
    let mut registry = FnRegistry::default();
    for def in defs.into_values() {
        registry.insert(def);
    }
    let result = check_fn_with_options(
        &conflict,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    );
    assert!(matches!(
        result,
        Err(wavelet_elab::TypeError::TypeMismatch { .. })
    ));
}

#[test]
fn shadowed_literals_are_inferred_independently() {
    let def = parse_fn_by_name(
        "fn shadowed() -> i32 { let x = 0; let x = 1; x }",
        "shadowed",
    );
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let typed = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    )
    .unwrap();
    let Stmt::LetVal { var: first, .. } = &typed.body.stmts[0] else {
        panic!("expected first literal");
    };
    let Stmt::LetVal { var: second, .. } = &typed.body.stmts[1] else {
        panic!("expected second literal");
    };
    assert_eq!(first.ty, Ty::Int(Signedness::Unsigned));
    assert_eq!(second.ty, Ty::Int(Signedness::Signed));
}

#[test]
fn mixed_signedness_comparison_is_rejected() {
    let def = parse_fn_by_name(
        "fn invalid(x: i32, y: u32) -> bool { let c = x < y; c }",
        "invalid",
    );
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let result = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    );
    assert!(matches!(
        result,
        Err(wavelet_elab::TypeError::TypeMismatch { .. })
    ));
}

#[test]
fn mixed_signedness_store_is_rejected_before_capability_checking() {
    let def = parse_fn_by_name(
        "#[cap(A: uniq @ 0..1)] fn invalid(x: u32, A: &mut [i32; 1]) { A[0] = x; () }",
        "invalid",
    );
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let result = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    );
    assert!(matches!(
        result,
        Err(wavelet_elab::TypeError::TypeMismatch { .. })
    ));
}

#[test]
fn shift_count_may_have_independent_signedness() {
    let def = parse_fn_by_name(
        "fn shifted(x: i32, amount: usize) -> i32 { let y = x >> amount; y }",
        "shifted",
    );
    let mut registry = FnRegistry::default();
    registry.insert(def.clone());
    let result = check_fn_with_options(
        &def,
        &registry,
        &SyntacticLogic::default(),
        CheckOptions::default(),
    );
    assert!(
        result.is_ok(),
        "shift count is independently typed: {result:?}"
    );
}
