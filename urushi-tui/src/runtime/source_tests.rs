use super::*;

#[test]
fn a_key_is_a_press_of_no_modifier_unless_it_says_otherwise() {
    let key = KeyEvent::new(KeyCode::Char('k'));

    assert_eq!(key.kind, KeyKind::Press);
    assert_eq!(key.modifiers, Modifiers::NONE);
    assert_eq!(
        KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CONTROL).modifiers,
        Modifiers::CONTROL
    );
}

#[test]
fn a_repeat_and_a_release_are_events_of_their_own() {
    let press = KeyEvent::new(KeyCode::Down);
    let repeat = KeyEvent {
        kind: KeyKind::Repeat,
        ..press
    };
    let release = KeyEvent {
        kind: KeyKind::Release,
        ..press
    };

    assert_ne!(press, repeat);
    assert_ne!(press, release);
    assert_eq!(press.code, repeat.code);
}

#[test]
fn modifiers_combine_and_say_which_are_held() {
    let control_alt = Modifiers::CONTROL.union(Modifiers::ALT);

    assert!(control_alt.control && control_alt.alt && !control_alt.shift);
    assert_eq!(format!("{control_alt:?}"), "Modifiers(control+alt)");
    assert_eq!(format!("{:?}", Modifiers::NONE), "Modifiers(none)");
}

#[test]
fn a_surface_reports_pixel_geometry_only_where_the_terminal_did() {
    let surface = Surface::new(80, 24);

    assert_eq!(
        surface.size,
        SurfaceSize {
            columns: 80,
            rows: 24
        }
    );
    assert_eq!(surface.cell_pixels, None);
    assert_eq!(
        Surface {
            cell_pixels: Some(CellPixels {
                width: 8,
                height: 16
            }),
            ..surface
        }
        .size,
        surface.size
    );
}
