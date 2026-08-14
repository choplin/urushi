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
        "VERTICAL ALIGNMENT",
        "HORIZONTAL ALIGNMENT",
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
    for position in ["TOP", "CENTER", "BOTTOM", "LEFT", "RIGHT"] {
        assert!(
            output.contains(position),
            "missing alignment name: {position}"
        );
    }

    let lines: Vec<_> = output.lines().collect();
    let fixed_width = lines
        .iter()
        .position(|line| line.contains("FIXED WIDTH"))
        .expect("fixed width row");
    let fixed_height = lines
        .iter()
        .position(|line| line.contains("FIXED HEIGHT"))
        .expect("fixed height row");
    let horizontal_alignment = lines
        .iter()
        .position(|line| line.contains("HORIZONTAL ALIGNMENT"))
        .expect("horizontal alignment showcase");
    let vertical_alignment = lines
        .iter()
        .position(|line| line.contains("VERTICAL ALIGNMENT"))
        .expect("vertical alignment showcase");
    assert_eq!(fixed_height - fixed_width, 2);
    assert_eq!(vertical_alignment - fixed_height, 6);
    assert_eq!(horizontal_alignment - vertical_alignment, 6);
}
