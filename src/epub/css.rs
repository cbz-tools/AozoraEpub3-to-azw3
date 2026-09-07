use crate::book::{CssDeclaration, CssRule, StyleSheet};

pub fn parse_css(href: impl Into<String>, source: impl Into<String>) -> StyleSheet {
    let href = href.into();
    let source = source.into();
    let without_comments = remove_comments(&source);
    let mut rules = Vec::new();
    for part in without_comments.split('}') {
        let Some((selector, declarations)) = part.split_once('{') else {
            continue;
        };
        let selector = selector.trim();
        if selector.is_empty() {
            continue;
        }
        let declarations = declarations
            .split(';')
            .filter_map(|declaration| {
                let (property, value) = declaration.split_once(':')?;
                let property = property.trim().to_ascii_lowercase();
                let value = value.trim().to_owned();
                (!property.is_empty() && !value.is_empty())
                    .then_some(CssDeclaration { property, value })
            })
            .collect();
        rules.push(CssRule {
            selector: selector.to_owned(),
            declarations,
        });
    }
    StyleSheet {
        href,
        source,
        rules,
    }
}

fn remove_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("/*") {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start + 2..].find("*/") else {
            break;
        };
        rest = &rest[start + 2 + end + 2..];
    }
    result.push_str(rest);
    result
}
