use bdslib::vm::helpers::eval::{bund_compile_and_eval, dynamic_to_json};
use bdslib::vm::vm::init_stdlib;
use bundcore::bundcore::Bund;

fn make_vm() -> Bund {
    let mut bund = Bund::new();
    init_stdlib(&mut bund).expect("stdlib init");
    bund
}

fn workbench_as_json(bund: &Bund) -> Vec<serde_json::Value> {
    bund.vm.stack.workbench.stack.iter().cloned().map(dynamic_to_json).collect()
}

#[test]
fn test_push_string_to_workbench() {
    let mut bund = make_vm();
    bund_compile_and_eval(&mut bund.vm, r#""Hello world" ."#.to_string())
        .expect("eval");
    let wb = workbench_as_json(&bund);
    assert_eq!(wb.len(), 1);
    assert_eq!(wb[0], serde_json::json!("Hello world"));
}

#[test]
fn test_push_integer_to_workbench() {
    let mut bund = make_vm();
    bund_compile_and_eval(&mut bund.vm, "42 .".to_string()).expect("eval");
    let wb = workbench_as_json(&bund);
    assert_eq!(wb.len(), 1);
    assert_eq!(wb[0], serde_json::json!(42));
}

#[test]
fn test_push_float_to_workbench() {
    let mut bund = make_vm();
    bund_compile_and_eval(&mut bund.vm, "3.14 .".to_string()).expect("eval");
    let wb = workbench_as_json(&bund);
    assert_eq!(wb.len(), 1);
    // JSON floats: compare as f64 with tolerance
    let v = wb[0].as_f64().expect("float");
    assert!((v - 3.14).abs() < 1e-10);
}

#[test]
fn test_push_bool_to_workbench() {
    let mut bund = make_vm();
    bund_compile_and_eval(&mut bund.vm, "true .".to_string()).expect("eval");
    let wb = workbench_as_json(&bund);
    assert_eq!(wb.len(), 1);
    assert_eq!(wb[0], serde_json::json!(true));
}

#[test]
fn test_push_list_to_workbench() {
    let mut bund = make_vm();
    bund_compile_and_eval(&mut bund.vm, "[ 1 2 3 ] .".to_string()).expect("eval");
    let wb = workbench_as_json(&bund);
    assert_eq!(wb.len(), 1);
    assert_eq!(wb[0], serde_json::json!([1, 2, 3]));
}

#[test]
fn test_push_arithmetic_result_to_workbench() {
    let mut bund = make_vm();
    bund_compile_and_eval(&mut bund.vm, "6 7 * .".to_string()).expect("eval");
    let wb = workbench_as_json(&bund);
    assert_eq!(wb.len(), 1);
    assert_eq!(wb[0], serde_json::json!(42));
}

#[test]
fn test_push_multiple_values_workbench_order() {
    let mut bund = make_vm();
    // Push three values in sequence; workbench preserves insertion order
    bund_compile_and_eval(
        &mut bund.vm,
        r#""first" . 2 . true ."#.to_string(),
    )
    .expect("eval");
    let wb = &bund.vm.stack.workbench.stack;
    assert_eq!(wb.len(), 3);
    assert_eq!(wb[0].cast_string().expect("string"), "first");
    assert_eq!(wb[1].cast_int().expect("int"), 2);
    assert_eq!(wb[2].cast_bool().expect("bool"), true);
}

#[test]
fn test_cast_methods_directly() {
    let mut bund = make_vm();
    bund_compile_and_eval(&mut bund.vm, r#"99 "hello" false ."#.to_string())
        .expect("eval");
    // `.` pops only TOS (false) to workbench; 99 and "hello" stay on the main stack.
    let wb = &bund.vm.stack.workbench.stack;
    assert_eq!(wb.len(), 1);
    assert_eq!(wb[0].cast_bool().expect("bool"), false);
}
