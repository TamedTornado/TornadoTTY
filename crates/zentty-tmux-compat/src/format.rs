use std::collections::BTreeMap;

pub struct FormatRenderer;

impl FormatRenderer {
    #[must_use]
    pub fn render(template: &str, context: &BTreeMap<String, String>) -> String {
        render_template(template, context)
    }
}

fn render_template(template: &str, context: &BTreeMap<String, String>) -> String {
    let mut result = String::new();
    let mut characters = template.chars();
    while let Some(character) = characters.next() {
        if character != '#' {
            result.push(character);
            continue;
        }
        match characters.next() {
            None | Some('#') => result.push('#'),
            Some('{') => {
                let body = brace_body(&mut characters);
                result.push_str(&expand(&body, context));
            }
            Some(token) => {
                if let Some(name) = short_token_name(token) {
                    result.push_str(context.get(name).map_or("", String::as_str));
                } else {
                    result.push('#');
                    result.push(token);
                }
            }
        }
    }
    result
}

fn brace_body(characters: &mut impl Iterator<Item = char>) -> String {
    let mut depth = 1;
    let mut body = String::new();
    for character in characters {
        if character == '{' {
            depth += 1;
        } else if character == '}' {
            depth -= 1;
            if depth == 0 {
                return body;
            }
        }
        body.push(character);
    }
    body
}

fn expand(body: &str, context: &BTreeMap<String, String>) -> String {
    body.strip_prefix('?').map_or_else(
        || context.get(body).cloned().unwrap_or_default(),
        |conditional| expand_conditional(conditional, context),
    )
}

fn expand_conditional(body: &str, context: &BTreeMap<String, String>) -> String {
    let parts = split_top_level(body);
    if parts.len() != 3 {
        return String::new();
    }
    let branch = if context.get(&parts[0]).is_none_or(String::is_empty) {
        &parts[2]
    } else {
        &parts[1]
    };
    render_template(branch, context)
}

fn split_top_level(source: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    for character in source.chars() {
        match character {
            '{' => {
                depth += 1;
                current.push(character);
            }
            '}' => {
                depth -= 1;
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(current);
                current = String::new();
            }
            _ => current.push(character),
        }
    }
    parts.push(current);
    parts
}

fn short_token_name(token: char) -> Option<&'static str> {
    match token {
        'S' => Some("session_name"),
        'I' => Some("window_index"),
        'P' => Some("pane_index"),
        'D' => Some("pane_id"),
        'T' => Some("pane_title"),
        'W' => Some("window_name"),
        'F' => Some("window_flags"),
        'H' => Some("host_short"),
        'h' => Some("host"),
        _ => None,
    }
}
