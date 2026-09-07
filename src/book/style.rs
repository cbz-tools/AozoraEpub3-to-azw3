use super::{Direction, WritingMode};

#[derive(Debug, Clone, Default)]
pub struct Styles {
    pub sheets: Vec<StyleSheet>,
    pub computed: Vec<ComputedStyle>,
}

#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
    // The parser retains the source identity and bytes for CSS transport;
    // semantic style projection consumes the parsed rules separately.
    #[allow(dead_code)]
    pub href: String,
    #[allow(dead_code)]
    pub source: String,
    pub rules: Vec<CssRule>,
}

#[derive(Debug, Clone, Default)]
pub struct CssRule {
    pub selector: String,
    pub declarations: Vec<CssDeclaration>,
}

#[derive(Debug, Clone, Default)]
pub struct CssDeclaration {
    pub property: String,
    pub value: String,
}

#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    // Selector identity belongs to the semantic style graph; current layout
    // normalization consumes only the recognized declaration values.
    #[allow(dead_code)]
    pub selector: String,
    pub writing_mode: Option<WritingMode>,
    pub direction: Option<Direction>,
    pub text_orientation: Option<String>,
    pub text_combine_upright: Option<String>,
    pub text_emphasis: Option<String>,
    pub ruby_position: Option<String>,
    pub ruby_align: Option<String>,
    pub line_break: Option<String>,
    pub word_break: Option<String>,
}

impl ComputedStyle {
    pub fn from_rule(rule: &CssRule) -> Self {
        let mut style = Self {
            selector: rule.selector.clone(),
            ..Self::default()
        };
        for declaration in &rule.declarations {
            let property = declaration.property.as_str();
            let value = declaration.value.trim();
            match property {
                "writing-mode" => {
                    style.writing_mode = match value {
                        "vertical-rl" => Some(WritingMode::VerticalRl),
                        "vertical-lr" => Some(WritingMode::VerticalLr),
                        "horizontal-tb" => Some(WritingMode::HorizontalTb),
                        _ => None,
                    }
                }
                "direction" => {
                    style.direction = match value {
                        "ltr" => Some(Direction::Ltr),
                        "rtl" => Some(Direction::Rtl),
                        _ => None,
                    }
                }
                "text-orientation" => style.text_orientation = Some(value.to_owned()),
                "text-combine-upright" => style.text_combine_upright = Some(value.to_owned()),
                "text-emphasis" | "-webkit-text-emphasis" => {
                    style.text_emphasis = Some(value.to_owned())
                }
                "ruby-position" => style.ruby_position = Some(value.to_owned()),
                "ruby-align" => style.ruby_align = Some(value.to_owned()),
                "line-break" => style.line_break = Some(value.to_owned()),
                "word-break" => style.word_break = Some(value.to_owned()),
                _ => {}
            }
        }
        style
    }
}

impl StyleSheet {
    pub fn computed_styles(&self) -> impl Iterator<Item = ComputedStyle> + '_ {
        self.rules.iter().map(ComputedStyle::from_rule)
    }
}
