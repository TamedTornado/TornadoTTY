pub struct SendKeys;

impl SendKeys {
    #[must_use]
    pub fn translate(arguments: &[String], standard_input: Option<&str>) -> String {
        let mut skip_next = false;
        let mut literal = false;
        let mut tokens = Vec::new();
        for argument in arguments {
            if skip_next {
                skip_next = false;
            } else if matches!(argument.as_str(), "-t" | "-T" | "-N") {
                skip_next = true;
            } else if argument == "-l" {
                literal = true;
            } else if !argument.starts_with('-') {
                tokens.push(argument.as_str());
            }
        }
        let translated = translate_tokens(&tokens, literal);
        if translated.is_empty() {
            standard_input.unwrap_or_default().to_owned()
        } else {
            translated
        }
    }
}

fn translate_tokens(tokens: &[&str], literal: bool) -> String {
    if literal {
        return tokens.join(" ");
    }
    let mut result = String::new();
    let mut pending_space = false;
    for token in tokens {
        if let Some(special) = special_key(token) {
            result.push(special);
            pending_space = false;
        } else {
            if pending_space {
                result.push(' ');
            }
            result.push_str(token);
            pending_space = true;
        }
    }
    result
}

fn special_key(token: &str) -> Option<char> {
    match token.to_ascii_lowercase().as_str() {
        "enter" | "c-m" | "kpenter" => Some('\r'),
        "tab" | "c-i" => Some('\t'),
        "space" => Some(' '),
        "bspace" | "backspace" => Some('\u{7f}'),
        "escape" | "esc" | "c-[" => Some('\u{1b}'),
        "c-c" => Some('\u{03}'),
        "c-d" => Some('\u{04}'),
        "c-z" => Some('\u{1a}'),
        "c-l" => Some('\u{0c}'),
        _ => None,
    }
}
