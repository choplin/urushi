use std::any::Any;

use crate::{
    FieldKey, Form, Group,
    runtime::{
        self, FieldAction, FieldEntry, FieldPresentation, PromptLine, PromptStyles, RuntimeField,
        TextSpan,
    },
};

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

    fn view(&self, styles: &PromptStyles, _focused: bool, _width: usize) -> FieldPresentation {
        FieldPresentation::new(
            PromptLine::spans(vec![TextSpan::new(
                self.key.name().to_owned(),
                styles.body.clone(),
            )])
            .view,
        )
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
