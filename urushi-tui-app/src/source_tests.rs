use super::*;

#[test]
fn a_key_is_a_press_of_no_modifier_unless_it_says_otherwise() {
    let key = KeyEvent::new(KeyCode::Char('k'));

    assert_eq!(key.kind, KeyKind::Press);
    assert_eq!(key.modifiers, Modifiers::NONE);
    assert_eq!(
        KeyEvent::new(KeyCode::Char('c'))
            .with_modifiers(Modifiers::CONTROL)
            .modifiers,
        Modifiers::CONTROL
    );
}

#[test]
fn a_repeat_and_a_release_are_events_of_their_own() {
    let press = KeyEvent::new(KeyCode::Down);
    let repeat = press.with_kind(KeyKind::Repeat);
    let release = press.with_kind(KeyKind::Release);

    assert_ne!(press, repeat);
    assert_ne!(press, release);
    assert_eq!(press.code, repeat.code);
}

#[test]
fn modifiers_combine_and_say_which_are_held() {
    let control_alt = Modifiers::CONTROL.union(Modifiers::ALT);

    assert!(control_alt.contains(Modifiers::CONTROL));
    assert!(control_alt.contains(Modifiers::ALT));
    assert!(!control_alt.contains(Modifiers::SHIFT));
    assert_eq!(format!("{control_alt:?}"), "Modifiers(control+alt)");
    assert_eq!(format!("{:?}", Modifiers::NONE), "Modifiers(none)");
}

#[test]
fn a_surface_reports_pixel_geometry_only_where_the_terminal_did() {
    let surface = Surface::new(80, 24);

    assert_eq!(surface.size, SurfaceSize::new(80, 24));
    assert_eq!(surface.cell_pixels, None);
    assert_eq!(
        Surface {
            cell_pixels: Some(CellPixels::new(8, 16)),
            ..surface
        }
        .size,
        surface.size
    );
}

#[test]
fn a_surface_derives_each_cell_size_from_window_geometry() {
    use urushi_terminal::{PixelSize, WindowSize};

    assert_eq!(
        Surface::from_window_size(WindowSize::new(
            SurfaceSize::new(80, 24),
            Some(PixelSize::new(640, 384)),
        )),
        Surface {
            size: SurfaceSize::new(80, 24),
            cell_pixels: Some(CellPixels::new(8, 16)),
        }
    );
    assert_eq!(
        Surface::from_window_size(WindowSize::new(
            SurfaceSize::ZERO,
            Some(PixelSize::new(640, 384)),
        )),
        Surface::new(0, 0)
    );
    assert_eq!(
        Surface::from_window_size(WindowSize::new(
            SurfaceSize::new(80, 24),
            Some(PixelSize::new(801, 480)),
        )),
        Surface::new(80, 24)
    );
}
