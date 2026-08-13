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
}
