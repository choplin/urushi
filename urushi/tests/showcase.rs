#[path = "../examples/showcase.rs"]
mod showcase;

use urushi::visible_width;

fn strip_csi(input: &str) -> String {
    let mut output = String::new();
    let mut characters = input.chars();
    while let Some(character) = characters.next() {
        if character == '\x1b' && characters.as_str().starts_with('[') {
            characters.next();
            for control in characters.by_ref() {
                if ('@'..='~').contains(&control) {
                    break;
                }
            }
        } else {
            output.push(character);
        }
    }
    output
}

#[test]
fn showcase_has_a_consistent_visible_width() {
    let output = showcase::render_showcase();
    let widths: Vec<usize> = output.lines().map(visible_width).collect();

    assert!(
        widths.iter().all(|width| *width == 49),
        "showcase lines should have the same visible width\n{output}"
    );
    assert!(
        output
            .lines()
            .next()
            .is_some_and(|line| line.contains("URUSHI STYLE SHOWCASE"))
    );
    for label in [
        "COLOR & TEXT",
        "BOX MODEL",
        "ALIGNMENT",
        "COMPOSITION",
        "COMPONENTS",
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
        "TREE",
    ] {
        assert!(output.contains(label), "missing feature label: {label}");
    }
    for tree_content in [
        "urushi",
        "├── ",
        "│   ",
        "└── ",
        "src",
        "theme",
        "Cargo.toml",
    ] {
        assert!(
            output.contains(tree_content),
            "missing Tree showcase content: {tree_content}"
        );
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
    let section_positions = [
        "COLOR & TEXT",
        "BOX MODEL",
        "ALIGNMENT",
        "COMPOSITION",
        "COMPONENTS",
    ]
    .map(|title| {
        lines
            .iter()
            .position(|line| line.contains(title))
            .unwrap_or_else(|| panic!("missing showcase section: {title}"))
    });
    assert!(
        section_positions.windows(2).all(|pair| pair[0] < pair[1]),
        "showcase sections should remain in reading order\n{output}"
    );

    let plain_lines = output.lines().map(strip_csi).collect::<Vec<_>>();
    let divider_column = |label: &str| {
        plain_lines
            .iter()
            .find(|line| line.contains(label))
            .and_then(|line| line.find(" | "))
            .unwrap_or_else(|| panic!("missing divider for showcase row: {label}"))
    };
    let center_divider = divider_column("FOREGROUND COLOR");
    assert_eq!(divider_column("JOIN HORIZONTAL"), center_divider);
    assert_eq!(divider_column("TREE"), center_divider);

    let vertical_alignment = plain_lines
        .iter()
        .position(|line| line.contains("VERTICAL ALIGNMENT"))
        .expect("vertical alignment row");
    let horizontal_alignment = plain_lines
        .iter()
        .position(|line| line.contains("HORIZONTAL ALIGNMENT"))
        .expect("horizontal alignment row");
    assert_eq!(horizontal_alignment - vertical_alignment, 6);
}
