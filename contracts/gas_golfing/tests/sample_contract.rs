use gas_golfing::gas_monitored;

#[gas_monitored]
fn compute(value: i32) -> i32 {
    value * 2 + 1
}

#[test]
fn gas_monitored_logs_duration_for_sample_function() {
    let result = compute(21);

    assert_eq!(result, 43);
}
