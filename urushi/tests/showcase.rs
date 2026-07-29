#[path = "../examples/showcase.rs"]
mod showcase;

use urushi::visible_width;

#[test]
fn showcase_has_a_consistent_visible_width() {
    let output = showcase::render_showcase();
    let widths: Vec<usize> = output.lines().map(visible_width).collect();

    assert!(
        widths.iter().all(|&width| width == widths[0]),
        "showcase lines should have the same visible width, got {widths:?}\n{output}"
    );
}
