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
    let plain_output = strip_csi(&output);
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
        "MAX WIDTH",
        "MAX HEIGHT",
        "VERTICAL ALIGNMENT",
        "HORIZONTAL ALIGNMENT",
        "JOIN HORIZONTAL",
        "JOIN VERTICAL",
        "BORDER PRESETS",
        "BORDER TOP ONLY",
        "BORDER RIGHT ONLY",
        "BORDER BOTTOM ONLY",
        "BORDER LEFT ONLY",
        "TREE",
        "LIST",
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
    for list_content in [
        "1. Define the API",
        "2. Implement",
        "model",
        "view",
        "3. Verify behavior",
    ] {
        assert!(
            plain_output.contains(list_content),
            "missing List showcase content: {list_content}"
        );
    }
    assert!(output.contains("reserves 20 cols"));
    assert!(output.contains("reserves 3 rows"));
    assert!(output.contains("┌───┬───┐"));
    assert!(output.contains("├───┼───┤"));
    assert!(output.contains("└───┴───┘"));
    assert!(output.contains("+---+---+"));
    assert!(output.contains("|---|---|"));
    assert!(output.contains("━━━━━━━━━"));
    assert!(output.contains("─────────"));
    for preset in [
        "NORMAL", "ROUNDED", "THICK", "DOUBLE", "ASCII", "MARKDOWN", "BOOKTABS", "HIDDEN",
    ] {
        assert!(output.contains(preset), "missing border preset: {preset}");
    }
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
    assert_eq!(divider_column("LIST"), center_divider);

    let vertical_alignment = plain_lines
        .iter()
        .position(|line| line.contains("VERTICAL ALIGNMENT"))
        .expect("vertical alignment row");
    let horizontal_alignment = plain_lines
        .iter()
        .position(|line| line.contains("HORIZONTAL ALIGNMENT"))
        .expect("horizontal alignment row");
    assert_eq!(horizontal_alignment - vertical_alignment, 6);

    let feature_position = |label: &str| {
        plain_lines
            .iter()
            .position(|line| line.contains(label))
            .unwrap_or_else(|| panic!("missing showcase feature: {label}"))
    };
    let fixed_width = feature_position("FIXED WIDTH");
    let fixed_height = feature_position("FIXED HEIGHT");
    let max_width = feature_position("MAX WIDTH");
    let max_height = feature_position("MAX HEIGHT");
    assert_eq!(fixed_height - fixed_width, 2);
    assert_eq!(max_width - fixed_height, 4);
    assert_eq!(max_height - max_width, 5);

    assert!(plain_lines[max_width + 1].contains("abcdefghijklmnopqrst"));
    assert!(plain_lines[max_width + 3].contains("abcdefghijkl"));
    assert!(!plain_lines[max_width + 3].contains('m'));
    for offset in 1..=3 {
        let letter = char::from(b'a' + (offset - 1) as u8);
        assert_eq!(
            plain_lines[max_height + offset].matches(letter).count(),
            2,
            "{letter} should appear in before and after boxes"
        );
    }
    for offset in 4..=6 {
        let letter = char::from(b'a' + (offset - 1) as u8);
        assert_eq!(
            plain_lines[max_height + offset].matches(letter).count(),
            1,
            "{letter} should only remain in the before box"
        );
    }
}
