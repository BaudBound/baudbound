use crate::runtime::evaluate_calculation_expression;

#[test]
fn evaluates_every_documented_operator_and_function() {
    let cases = [
        ("1 + 2", 3.0),
        ("7 - 4", 3.0),
        ("3 * 4", 12.0),
        ("8 / 4", 2.0),
        ("8 % 3", 2.0),
        ("2 ^ 3", 8.0),
        ("2 + 3 * 4", 14.0),
        ("(2 + 3) * 4", 20.0),
        ("-2 ^ 2", -4.0),
        ("2 ^ -2", 0.25),
        ("1.5e2 + .5", 150.5),
        ("round(1.5)", 2.0),
        ("round(-1.5)", -1.0),
        ("floor(1.9)", 1.0),
        ("ceil(1.1)", 2.0),
        ("min(4, -2, 8)", -2.0),
        ("max(4, -2, 8)", 8.0),
    ];

    for (expression, expected) in cases {
        assert_eq!(
            evaluate_calculation_expression(expression),
            Ok(expected),
            "unexpected result for {expression}"
        );
    }
}

#[test]
fn random_function_respects_all_documented_ranges() {
    for _ in 0..64 {
        let unit = evaluate_calculation_expression("random()").expect("random() should evaluate");
        assert!((0.0..1.0).contains(&unit));

        let maximum =
            evaluate_calculation_expression("random(10)").expect("random(max) should evaluate");
        assert!((0.0..10.0).contains(&maximum));

        let range = evaluate_calculation_expression("random(5, 15)")
            .expect("random(min, max) should evaluate");
        assert!((5.0..15.0).contains(&range));
    }
}

#[test]
fn rejects_malformed_or_unsafe_calculation_expressions() {
    let expressions = [
        "",
        "1 / 0",
        "1 % 0",
        "2 ^ 1024",
        "round()",
        "floor(1, 2)",
        "min()",
        "max()",
        "random(1, 2, 3)",
        "unknown(1)",
        "round",
        "(1 + 2",
        "1 + 2)",
        "1 +",
        "1 2",
        "1 + @",
        ".",
        "1e999",
    ];

    for expression in expressions {
        let error = evaluate_calculation_expression(expression)
            .expect_err("malformed calculation must fail");
        assert!(
            !error.trim().is_empty(),
            "{expression:?} should produce an actionable error"
        );
    }
}
