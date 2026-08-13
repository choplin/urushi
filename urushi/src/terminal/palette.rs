//! Deterministic xterm palette conversion.

use crate::Color;

pub(super) fn quantize_to_ansi256(color: Color) -> Color {
    match color {
        Color::Ansi(_) | Color::Ansi256(_) => color,
        Color::Rgb(r, g, b) => color_from_index(nearest_palette_index((r, g, b), 256)),
    }
}

pub(super) fn quantize_to_ansi16(color: Color) -> Color {
    Color::Ansi(nearest_palette_index(color_to_rgb(color), 16))
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Ansi(index) | Color::Ansi256(index) => xterm_rgb(index),
    }
}

fn color_from_index(index: u8) -> Color {
    if index < 16 {
        Color::Ansi(index)
    } else {
        Color::Ansi256(index)
    }
}

fn nearest_palette_index(rgb: (u8, u8, u8), palette_size: u16) -> u8 {
    let mut nearest = 0;
    let mut nearest_distance = squared_distance(rgb, xterm_rgb(0));
    for index in 1..palette_size {
        let index = index as u8;
        let distance = squared_distance(rgb, xterm_rgb(index));
        if distance < nearest_distance {
            nearest = index;
            nearest_distance = distance;
        }
    }
    nearest
}

fn squared_distance((r, g, b): (u8, u8, u8), (cr, cg, cb): (u8, u8, u8)) -> u32 {
    let dr = i32::from(r) - i32::from(cr);
    let dg = i32::from(g) - i32::from(cg);
    let db = i32::from(b) - i32::from(cb);
    (dr * dr + dg * dg + db * db) as u32
}

fn xterm_rgb(index: u8) -> (u8, u8, u8) {
    const ANSI: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0..=15 => ANSI[usize::from(index)],
        16..=231 => {
            let cube = index - 16;
            (
                CUBE[usize::from(cube / 36)],
                CUBE[usize::from((cube % 36) / 6)],
                CUBE[usize::from(cube % 6)],
            )
        }
        232..=255 => {
            let gray = 8 + 10 * (index - 232);
            (gray, gray, gray)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizes_xterm_colors_deterministically() {
        assert_eq!(
            quantize_to_ansi256(Color::Rgb(95, 135, 175)),
            Color::Ansi256(67)
        );
        assert_eq!(quantize_to_ansi256(Color::Rgb(0, 0, 0)), Color::Ansi(0));
        assert_eq!(
            quantize_to_ansi256(Color::Rgb(128, 128, 128)),
            Color::Ansi(8)
        );
        assert_eq!(quantize_to_ansi16(Color::Rgb(255, 0, 0)), Color::Ansi(9));
        assert_eq!(quantize_to_ansi16(Color::Ansi256(16)), Color::Ansi(0));
    }
}
