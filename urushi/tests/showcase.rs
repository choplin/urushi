#[path = "../examples/showcase.rs"]
mod showcase;

use urushi::visible_width;

#[test]
fn showcase_has_a_consistent_visible_width() {
    let output = showcase::render_showcase();
    let widths: Vec<usize> = output.lines().map(visible_width).collect();
    let margin_row = " ".repeat(49);

    assert!(
        widths.iter().all(|width| *width == 49),
        "showcase lines should have the same visible width\n{output}"
    );
    assert_eq!(output.lines().next(), Some(margin_row.as_str()));
    for label in [
        "FOREGROUND COLOR",
        "BACKGROUND COLOR",
        "BOLD",
        "UNDERLINE",
        "NESTED ANSI",
        "PADDING",
        "FULL BORDER",
        "FIXED WIDTH",
        "FIXED HEIGHT",
        "ALIGN LEFT",
        "ALIGN CENTER",
        "ALIGN RIGHT",
        "JOIN HORIZONTAL",
        "JOIN VERTICAL",
        "BORDER TOP ONLY",
        "BORDER RIGHT ONLY",
        "BORDER BOTTOM ONLY",
        "BORDER LEFT ONLY",
    ] {
        assert!(output.contains(label), "missing feature label: {label}");
    }
    assert!(output.contains("reserves 20 cols"));
    assert!(output.contains("reserves 3 rows"));

    let lines: Vec<_> = output.lines().collect();
    let fixed_width = lines
        .iter()
        .position(|line| line.contains("FIXED WIDTH"))
        .expect("fixed width row");
    let fixed_height = lines
        .iter()
        .position(|line| line.contains("FIXED HEIGHT"))
        .expect("fixed height row");
    let align_left = lines
        .iter()
        .position(|line| line.contains("ALIGN LEFT"))
        .expect("alignment group");
    assert_eq!(fixed_height - fixed_width, 2);
    assert_eq!(align_left - fixed_height, 4);
}
