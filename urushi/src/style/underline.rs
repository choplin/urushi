use urushi_terminal::UnderlineStyle;

pub(crate) const fn sgr_params(style: UnderlineStyle) -> &'static str {
    match style {
        UnderlineStyle::Single => "4",
        UnderlineStyle::Double => "4:2",
        UnderlineStyle::Curly => "4:3",
        UnderlineStyle::Dotted => "4:4",
        UnderlineStyle::Dashed => "4:5",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_has_its_own_sgr_parameter() {
        let params = [
            UnderlineStyle::Single,
            UnderlineStyle::Double,
            UnderlineStyle::Curly,
            UnderlineStyle::Dotted,
            UnderlineStyle::Dashed,
        ]
        .map(sgr_params);

        assert_eq!(params, ["4", "4:2", "4:3", "4:4", "4:5"]);
    }
}
