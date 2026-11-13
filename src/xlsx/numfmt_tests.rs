use super::*;

fn f(v: f64, code: &str) -> String {
    format(v, code, false)
}

#[test]
fn numbers_percent_grouping_and_sections() {
    assert_eq!(f(1234.5, "General"), "1234.5");
    assert_eq!(f(0.1 + 0.2, "General"), "0.3");
    assert_eq!(f(15322400.0, "#,##0"), "15,322,400");
    assert_eq!(f(0.279, "0.0%"), "27.9%");
    assert_eq!(f(-1234.567, "#,##0.00"), "-1,234.57");
    assert_eq!(f(-5.0, "#,##0_);(#,##0)"), "(5)");
    assert_eq!(f(0.0, "#,##0;-#,##0;\"-\""), "-");
    assert_eq!(f(3.0, "000"), "003");
    assert_eq!(f(12345678.0, "#,##0,,\"M\""), "12M");
    assert_eq!(f(1.5, "0.##"), "1.5");
    assert_eq!(f(2.0, "0.##"), "2.");
    assert_eq!(f(9.99, "[$£-809]#,##0.00"), "£9.99");
    assert_eq!(f(22.27, "0.00\"°\""), "22.27°");
    assert_eq!(f(12345.0, "0.00E+00"), "1.23E+04");
    assert_eq!(f(0.75, "# ?/?"), "3/4");
    assert_eq!(f(1.5, "# ?/?"), "1 1/2");
    assert_eq!(general(1e20), "1E+20");
    assert_eq!(f(122.0, "@"), "122");
    assert_eq!(f(0.0095, "0.0%"), "1.0%");
    assert_eq!(f(2.675, "0.00"), "2.68");
    for (v, d, want) in [
        (0.0, 2, "0.00"),
        (9.995, 2, "10.00"),
        (0.0004, 3, "0.000"),
        (0.0005, 3, "0.001"),
        (123456.0, 0, "123456"),
        (1e-7, 2, "0.00"),
    ] {
        assert_eq!(round_decimal(v, d), want, "{v} {d}");
    }
}
