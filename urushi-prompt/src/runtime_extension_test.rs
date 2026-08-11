use std::any::Any;

use crate::{
    FieldKey, Form, Group,
    runtime::{self, FieldAction, FieldEntry, PromptView, RuntimeField, ViewLine, ViewSpan},
};
use urushi::ComponentRole;

struct SiblingField {
    key: FieldKey<String>,
}

impl SiblingField {
    fn new(name: &str) -> Self {
        Self {
            key: FieldKey::new(name),
        }
    }
}

impl runtime::private::Sealed for SiblingField {
    fn into_entry(self: Box<Self>) -> FieldEntry {
        FieldEntry::new(self.key.name().to_owned(), self)
    }
}

impl RuntimeField for SiblingField {
    fn event(&mut self, _event: runtime::Event) -> FieldAction {
        FieldAction::Stay
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        Box::new(String::new())
    }

    fn view(&self) -> PromptView {
        PromptView {
            lines: vec![ViewLine {
                spans: vec![ViewSpan {
                    text: self.key.name().to_owned(),
                    role: ComponentRole::Body,
                }],
            }],
            cursor: None,
        }
    }
}

#[test]
fn crate_root_sibling_can_supply_a_sealed_runtime_field() {
    let group = Group::builder()
        .field(SiblingField::new("future-field"))
        .build()
        .expect("sibling field creates a group");
    let form = Form::builder()
        .group(group)
        .build()
        .expect("sibling field creates a form");

    drop(form);
}
