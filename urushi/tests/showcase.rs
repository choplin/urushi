#[path = "../examples/showcase.rs"]
mod showcase;

use urushi::visible_width;

#[test]
fn showcase_has_a_consistent_visible_width() {
    let output = showcase::render_showcase();
    let widths: Vec<usize> = output.lines().map(visible_width).collect();
    let margin_row = " ".repeat(46);

    assert_eq!(
        widths,
        vec![46; 15],
        "showcase lines should have the same visible width\n{output}"
    );
    assert_eq!(output.lines().next(), Some(margin_row.as_str()));
}
