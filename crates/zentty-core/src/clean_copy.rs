#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandFlattenAggressiveness {
    Low,
    Normal,
    High,
}

impl CommandFlattenAggressiveness {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }

    const fn score_threshold(self) -> usize {
        match self {
            Self::Low => 3,
            Self::Normal => 2,
            Self::High => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// These independent source-compatible feature switches are not mutually exclusive states.
#[allow(clippy::struct_excessive_bools)]
pub struct CleanCopyOptions {
    pub flatten_multi_line_commands: bool,
    pub command_flatten_aggressiveness: CommandFlattenAggressiveness,
    pub preserve_blank_lines_when_flattening: bool,
    pub remove_box_drawing: bool,
    pub flatten_slash_command_selections: bool,
    pub strip_url_tracking_parameters: bool,
    pub quote_paths_with_spaces: bool,
}

impl Default for CleanCopyOptions {
    fn default() -> Self {
        Self {
            flatten_multi_line_commands: true,
            command_flatten_aggressiveness: CommandFlattenAggressiveness::Normal,
            preserve_blank_lines_when_flattening: false,
            remove_box_drawing: true,
            flatten_slash_command_selections: true,
            strip_url_tracking_parameters: true,
            quote_paths_with_spaces: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanCopyResult {
    pub text: String,
    pub was_modified: bool,
}

#[must_use]
pub fn clean_copy(input: &str, options: CleanCopyOptions) -> CleanCopyResult {
    clean_copy_with_columns(input, options, None)
}

#[must_use]
pub fn clean_copy_with_columns(
    input: &str,
    options: CleanCopyOptions,
    columns: Option<usize>,
) -> CleanCopyResult {
    let mut text = strip_ansi(input);
    let wrap_width = WrapWidthEvidence::new(&text, columns);
    let padded_short_rows = has_multiple_padded_short_rows(&text);
    text = trim_trailing_whitespace(&text);
    text = trim_trailing_blank_lines(&text);
    text = strip_line_number_gutter(&text).unwrap_or(text);
    if options.remove_box_drawing {
        text = strip_box_chrome(&text).unwrap_or(text);
    }
    text = strip_agent_prompt(&text, padded_short_rows, wrap_width).unwrap_or(text);
    if options.flatten_slash_command_selections {
        text = strip_slash_command_decoration(&text).unwrap_or(text);
    }
    text = strip_prompt_prefixes(&text).unwrap_or(text);
    text = repair_wrapped_url(&text).unwrap_or(text);
    if options.strip_url_tracking_parameters {
        text = strip_tracking_parameters(&text).unwrap_or(text);
    }
    if options.quote_paths_with_spaces {
        text = quote_path_with_spaces(&text).unwrap_or(text);
    }
    if options.flatten_multi_line_commands {
        text = transform_multi_line_command(&text, options, padded_short_rows).unwrap_or(text);
    }
    text = reflow_plain_prose(&text, padded_short_rows, wrap_width).unwrap_or(text);
    text = dedent_common_prefix(&text);
    CleanCopyResult {
        was_modified: text != input,
        text,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WrapWidthEvidence {
    width: Option<usize>,
}

impl WrapWidthEvidence {
    fn new(input: &str, columns: Option<usize>) -> Self {
        let longest = input
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(|line| line.trim().chars().count())
            .max()
            .unwrap_or(0);
        let width = columns.unwrap_or(usize::MAX).min(longest);
        Self {
            width: (width > 0).then_some(width),
        }
    }

    fn ends_before_wrap_edge(self, line: &str) -> bool {
        let Some(width) = self.width else {
            return false;
        };
        let slack = 16.max(width / 4);
        line.trim().chars().count() < width.saturating_sub(slack)
    }
}

fn strip_ansi(input: &str) -> String {
    let mut characters = input.chars().peekable();
    let mut output = String::with_capacity(input.len());
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }
        match characters.peek().copied() {
            Some('[') => {
                characters.next();
                for candidate in characters.by_ref() {
                    if ('@'..='~').contains(&candidate) {
                        break;
                    }
                }
            }
            Some(']') => {
                characters.next();
                let mut saw_escape = false;
                for candidate in characters.by_ref() {
                    if candidate == '\u{7}' || (saw_escape && candidate == '\\') {
                        break;
                    }
                    saw_escape = candidate == '\u{1b}';
                }
            }
            Some('(') => {
                characters.next();
                characters.next();
            }
            Some('=') => {
                characters.next();
                for candidate in characters.by_ref() {
                    if candidate == '\n' {
                        output.push('\n');
                        break;
                    }
                }
            }
            Some(_) | None => output.push(character),
        }
    }
    output
}

fn trim_trailing_whitespace(input: &str) -> String {
    input
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

fn trim_trailing_blank_lines(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let had_newline = input.ends_with('\n');
    let mut lines = input.split('\n').collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut output = lines.join("\n");
    if had_newline {
        output.push('\n');
    }
    output
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum GutterKind {
    Tab,
    Colon,
    Pipe,
}

fn numbered_content(line: &str, kind: GutterKind) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let number = trimmed[..digits].parse().ok()?;
    let suffix = &trimmed[digits..];
    match kind {
        GutterKind::Tab => suffix.strip_prefix('\t').map(|content| (number, content)),
        GutterKind::Pipe => {
            let suffix = suffix.strip_prefix(' ').unwrap_or(suffix);
            let content = suffix.strip_prefix(['|', '│', '┃'])?;
            Some((number, content.strip_prefix(' ').unwrap_or(content)))
        }
        GutterKind::Colon => {
            let content = suffix.strip_prefix(':')?;
            let bytes = content.as_bytes();
            let two_digits =
                bytes.len() >= 2 && bytes[0].is_ascii_digit() && bytes[1].is_ascii_digit();
            let looks_like_time = two_digits && bytes.get(2).is_none_or(u8::is_ascii_whitespace);
            if looks_like_time {
                None
            } else {
                Some((number, content.strip_prefix(' ').unwrap_or(content)))
            }
        }
    }
}

fn strip_line_number_gutter(input: &str) -> Option<String> {
    let lines = input.split('\n').collect::<Vec<_>>();
    let nonempty = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if nonempty.is_empty()
        || nonempty.iter().all(|line| {
            line.trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::Ipv6Addr>()
                .is_ok()
        })
    {
        return None;
    }
    for kind in [GutterKind::Tab, GutterKind::Colon, GutterKind::Pipe] {
        let parsed_nonempty = nonempty
            .iter()
            .map(|line| numbered_content(line, kind))
            .collect::<Vec<_>>();
        let matched = parsed_nonempty.iter().filter(|item| item.is_some()).count();
        let qualifies = if nonempty.len() <= 3 {
            matched == nonempty.len()
        } else {
            matched * 5 > nonempty.len() * 4
                && parsed_nonempty
                    .iter()
                    .flatten()
                    .map(|(number, _)| *number)
                    .collect::<Vec<_>>()
                    .windows(2)
                    .all(|pair| pair[1] >= pair[0])
        };
        if qualifies {
            return Some(
                lines
                    .iter()
                    .map(|line| numbered_content(line, kind).map_or(*line, |(_, content)| content))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
    }
    None
}

fn is_border_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|character| "─━┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬╭╮╯╰┏┓┗┛┣┫┳┻╋".contains(character))
}

fn strip_box_chrome(input: &str) -> Option<String> {
    let lines = input.split('\n').collect::<Vec<_>>();
    if !input
        .chars()
        .any(|character| "│┃║╎╏┆┇┊┋╽╿￨｜─━┌┐└┘├┤┬┴┼═╔╗╚╝╠╣╦╩╬╭╮╯╰┏┓┗┛┣┫┳┻╋".contains(character))
    {
        return None;
    }
    let retained = lines
        .into_iter()
        .filter(|line| !is_border_line(line))
        .collect::<Vec<_>>();
    let nonempty = retained
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let threshold = if nonempty.len() == 1 {
        1
    } else {
        nonempty.len() / 2 + 1
    };
    let leading = nonempty
        .iter()
        .filter(|line| line.trim_start().starts_with(['│', '┃', '║']))
        .count()
        >= threshold;
    let trailing = nonempty
        .iter()
        .filter(|line| line.trim_end().ends_with(['│', '┃', '║']))
        .count()
        >= threshold;
    let has_inline_artifact = nonempty.iter().any(|line| {
        line.find([
            '│', '┃', '║', '╎', '╏', '┆', '┇', '┊', '┋', '╽', '╿', '￨', '｜',
        ])
        .is_some_and(|index| {
            !line[..index].trim().is_empty()
                && !line[index..]
                    .trim_start_matches([
                        '│', '┃', '║', '╎', '╏', '┆', '┇', '┊', '┋', '╽', '╿', '￨', '｜',
                    ])
                    .trim()
                    .is_empty()
        })
    });
    if !leading && !trailing && !has_inline_artifact {
        return None;
    }
    let mut output = Vec::with_capacity(retained.len());
    for line in retained {
        let mut content = line.to_owned();
        if leading {
            content = content
                .trim_start()
                .trim_start_matches(['│', '┃', '║'])
                .trim_start()
                .to_owned();
        }
        if trailing {
            content = content
                .trim_end()
                .trim_end_matches(['│', '┃', '║'])
                .trim_end()
                .to_owned();
        }
        content = clean_inline_box_artifacts(&content);
        output.push(content.trim_end().to_owned());
    }
    let cleaned = output.join("\n");
    if cleaned.chars().all(char::is_whitespace) && !input.chars().all(char::is_whitespace) {
        None
    } else {
        (cleaned != input).then_some(cleaned)
    }
}

fn clean_inline_box_artifacts(line: &str) -> String {
    let Some(index) = line.find([
        '│', '┃', '║', '╎', '╏', '┆', '┇', '┊', '┋', '╽', '╿', '￨', '｜',
    ]) else {
        return line.to_owned();
    };
    let before = line[..index].trim_end();
    let mut after = &line[index..];
    after = after.trim_start_matches([
        '│', '┃', '║', '╎', '╏', '┆', '┇', '┊', '┋', '╽', '╿', '￨', '｜',
    ]);
    after = after.trim_start();
    if before.is_empty() || after.is_empty() {
        return line.to_owned();
    }
    let separator = if before.ends_with([':', '/']) {
        ""
    } else {
        " "
    };
    clean_inline_box_artifacts(&format!("{before}{separator}{after}"))
}

fn prompt_content(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    for prefix in ["$ ", "# ", "% "] {
        if let Some(content) = trimmed.strip_prefix(prefix) {
            return Some(content);
        }
    }
    None
}

fn has_multiple_padded_short_rows(input: &str) -> bool {
    input
        .split('\n')
        .filter(|line| {
            let trailing = line
                .chars()
                .rev()
                .take_while(|character| matches!(character, ' ' | '\t'))
                .count();
            let visible = line.chars().count().saturating_sub(trailing);
            visible > 0 && visible < 60 && trailing >= 4
        })
        .take(2)
        .count()
        >= 2
}

fn agent_marker(line: &str) -> Option<char> {
    line.trim_start()
        .chars()
        .next()
        .filter(|character| matches!(character, '›' | '❯' | '•' | '⏺' | '●'))
}

fn strip_agent_prompt(
    input: &str,
    padded_short_rows: bool,
    wrap_width: WrapWidthEvidence,
) -> Option<String> {
    let lines = input.split('\n').collect::<Vec<_>>();
    let nonempty = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let first_marker = nonempty.first().and_then(|line| agent_marker(line))?;
    if nonempty.len() > 60 {
        return None;
    }
    let marker_count = nonempty
        .iter()
        .filter(|line| agent_marker(line).is_some())
        .count();
    if marker_count > 1 {
        if first_marker == '•' {
            return reflow_separated_bullets(&lines, wrap_width);
        }
        return None;
    }
    let rule_index = lines.iter().position(|line| is_agent_prompt_rule(line));
    let source_lines = rule_index.map_or(lines.as_slice(), |index| &lines[index + 1..]);
    let mut stripped_first = rule_index.is_some();
    let candidate = source_lines
        .iter()
        .map(|line| {
            if stripped_first || line.trim().is_empty() {
                return (*line).to_owned();
            }
            stripped_first = true;
            let trimmed = line.trim();
            trimmed
                .chars()
                .skip(1)
                .collect::<String>()
                .trim_start()
                .to_owned()
        })
        .collect::<Vec<_>>();
    if padded_short_rows {
        return Some(candidate.join("\n"));
    }
    let content = candidate.join("\n");
    if is_likely_source_code(&content)
        || is_likely_list(&content)
        || is_likely_structured_data(&content)
        || is_shell_transcript(&content)
    {
        return None;
    }
    let flattened = flatten_wrapped_paragraphs(&candidate, wrap_width);
    (flattened != input).then_some(flattened)
}

fn is_agent_prompt_rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.chars().count() >= 10
        && trimmed
            .chars()
            .all(|character| matches!(character, '─' | '━' | '—'))
}

fn strip_slash_command_decoration(input: &str) -> Option<String> {
    let nonempty = input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let first = *nonempty.first()?;
    let command_token = first
        .split_once([' ', '"'])
        .map_or(first, |(token, _)| token);
    let valid_command = command_token.strip_prefix('/').is_some_and(|name| {
        let mut pieces = name.split(':');
        let valid_piece = |piece: &str| {
            !piece.is_empty()
                && piece.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
        };
        valid_piece(pieces.next().unwrap_or_default())
            && pieces.next().is_none_or(valid_piece)
            && pieces.next().is_none()
    });
    if valid_command && nonempty.len() >= 2 {
        return Some(
            nonempty
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    let trimmed = input.trim();
    if trimmed.starts_with("\"/") && trimmed.ends_with('"') && trimmed.contains("\\\"") {
        return Some(trimmed[1..trimmed.len() - 1].replace("\\\"", "\""));
    }
    None
}

fn reflow_separated_bullets(lines: &[&str], wrap_width: WrapWidthEvidence) -> Option<String> {
    let blocks = lines
        .split(|line| line.trim().is_empty())
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>();
    if blocks.len() < 2
        || blocks.iter().any(|block| {
            block.first().and_then(|line| agent_marker(line)) != Some('•')
                || block
                    .iter()
                    .filter(|line| agent_marker(line).is_some())
                    .count()
                    != 1
        })
    {
        return None;
    }
    Some(
        blocks
            .into_iter()
            .map(|block| {
                let mut content = block.iter().map(|line| line.trim()).collect::<Vec<_>>();
                content[0] = content[0].trim_start_matches('•').trim_start();
                format!("• {}", flatten_paragraph(&content, wrap_width))
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

fn reflow_plain_prose(
    input: &str,
    padded_short_rows: bool,
    wrap_width: WrapWidthEvidence,
) -> Option<String> {
    let mut lines = input.split('\n').map(str::trim).collect::<Vec<_>>();
    while lines.first() == Some(&"") {
        lines.remove(0);
    }
    while lines.last() == Some(&"") {
        lines.pop();
    }
    let nonempty = lines
        .iter()
        .filter(|line| !line.is_empty())
        .copied()
        .collect::<Vec<_>>();
    let has_candidate = lines.split(|line| line.is_empty()).any(|paragraph| {
        paragraph.len() >= 2
            && !paragraph.iter().all(|line| is_list_item(line))
            && !paragraph.iter().all(|line| is_structured_record_line(line))
            && paragraph
                .iter()
                .any(|line| line.len() >= 60 && line.contains(' '))
    });
    if nonempty.len() < 2
        || nonempty.len() > 60
        || padded_short_rows
        || !has_candidate
        || is_compact_shell_block(&nonempty)
        || is_likely_source_code(input)
        || is_likely_structured_data(input)
        || is_shell_transcript(input)
    {
        return None;
    }
    let blockquotes = nonempty.iter().filter(|line| line.starts_with('>')).count();
    if blockquotes > nonempty.len() / 2 {
        lines = lines
            .into_iter()
            .map(|line| {
                if line.chars().all(|character| character == '>') {
                    ""
                } else {
                    line.trim_start_matches('>').trim_start()
                }
            })
            .collect();
    }
    let flattened = flatten_wrapped_paragraphs(&lines, wrap_width);
    (flattened != input).then_some(flattened)
}

fn flatten_wrapped_paragraphs<T: AsRef<str>>(lines: &[T], wrap_width: WrapWidthEvidence) -> String {
    let first = lines
        .iter()
        .position(|line| !line.as_ref().trim().is_empty())
        .unwrap_or(lines.len());
    let last = lines
        .iter()
        .rposition(|line| !line.as_ref().trim().is_empty())
        .map_or(first, |index| index + 1);
    let mut output = String::new();
    let mut paragraph = Vec::new();
    let mut blanks = 0;
    for line in &lines[first..last] {
        let line = line.as_ref().trim();
        if line.is_empty() {
            if !paragraph.is_empty() {
                if !output.is_empty() {
                    output.push_str(&"\n".repeat(blanks + 1));
                }
                output.push_str(&flatten_paragraph(&paragraph, wrap_width));
                paragraph.clear();
                blanks = 0;
            }
            blanks += 1;
        } else {
            paragraph.push(line);
        }
    }
    if !paragraph.is_empty() {
        if !output.is_empty() {
            output.push_str(&"\n".repeat(blanks + 1));
        }
        output.push_str(&flatten_paragraph(&paragraph, wrap_width));
    }
    output
}

fn flatten_paragraph(lines: &[&str], wrap_width: WrapWidthEvidence) -> String {
    if lines.iter().all(|line| is_structured_record_line(line)) {
        return lines.join("\n");
    }
    let mut output = Vec::new();
    let mut current = String::new();
    for (index, line) in lines.iter().enumerate() {
        let preceding_real_newline = index > 0
            && !lines[index - 1].trim_end().ends_with('\\')
            && wrap_width.ends_before_wrap_edge(lines[index - 1]);
        if (is_list_item(line) || preceding_real_newline) && !current.is_empty() {
            output.push(std::mem::take(&mut current));
        }
        if !current.is_empty() && !should_join_wrapped_token(&current, line) {
            current.push(' ');
        }
        current.push_str(line.trim_end_matches('\\').trim_end());
    }
    if !current.is_empty() {
        output.push(current);
    }
    output.join("\n")
}

fn is_structured_record_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with('#')
        || (trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2)
    {
        return true;
    }
    let key_length = trimmed
        .chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-')
        })
        .count();
    if key_length == 0
        || !trimmed
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
    {
        return false;
    }
    let remainder = trimmed[key_length..].trim_start();
    remainder.starts_with('=') || remainder.starts_with(": ")
}

fn should_join_wrapped_token(left: &str, right: &str) -> bool {
    let Some(right_token) = right.split_whitespace().next() else {
        return false;
    };
    let left_token = left.split_whitespace().last().unwrap_or_default();
    let right_first = right_token.chars().next().unwrap_or_default();
    if left_token.ends_with(['/', '~']) && is_path_token_character(right_first) {
        return true;
    }
    if left_token.ends_with('-')
        && !left_token.ends_with("--")
        && right_first.is_ascii_alphanumeric()
    {
        return true;
    }
    let identifier_character = |character: char| {
        character.is_ascii_uppercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '.')
    };
    let right_identifier = right_token
        .chars()
        .take_while(|character| identifier_character(*character))
        .collect::<String>();
    let left_fragment = left_token.len() == 1
        && left_token.chars().all(identifier_character)
        && right_identifier.contains('_')
        && right_identifier.chars().count() >= 2;
    let boundary_underscore = left_token.ends_with('_')
        && left_token.chars().all(identifier_character)
        && right_identifier.chars().count() >= 2;
    left_fragment || boundary_underscore
}

fn is_list_item(line: &str) -> bool {
    let trimmed = line.trim();
    ["- ", "* ", "• "]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        || trimmed
            .split_once(['.', ')'])
            .is_some_and(|(number, rest)| {
                !number.is_empty()
                    && number.chars().all(|character| character.is_ascii_digit())
                    && rest.starts_with(' ')
            })
}

fn is_likely_list(input: &str) -> bool {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .is_some_and(is_list_item)
}

fn is_likely_source_code(input: &str) -> bool {
    let has_braces = input.contains(['{', '}']) || input.to_ascii_lowercase().contains("begin");
    let keyword_line = input.lines().any(|line| {
        let first = line.split_whitespace().next().unwrap_or_default();
        [
            "import",
            "package",
            "namespace",
            "using",
            "template",
            "class",
            "struct",
            "enum",
            "extension",
            "protocol",
            "interface",
            "func",
            "def",
            "fn",
            "let",
            "var",
            "public",
            "private",
            "internal",
            "open",
            "protected",
            "if",
            "for",
            "while",
            "await",
            "try",
            "return",
            "guard",
        ]
        .contains(&first)
    });
    keyword_line && (has_braces || input.chars().any(|character| "=(){};".contains(character)))
}

fn is_likely_structured_data(input: &str) -> bool {
    input.lines().any(|line| {
        let trimmed = line.trim();
        ["{", "}", "[", "]"].contains(&trimmed)
            || ((trimmed.starts_with(['\'', '"'])) && trimmed.contains(':'))
    })
}

fn is_shell_transcript(input: &str) -> bool {
    input.lines().any(|line| {
        ["$ ", "# ", "% "]
            .iter()
            .any(|prefix| line.trim_start().starts_with(prefix))
    })
}

fn is_compact_shell_block(lines: &[&str]) -> bool {
    lines.len() >= 2
        && !lines.iter().any(|line| {
            ["--", "|", "&&", "||", "\\", "\"", "'"]
                .iter()
                .any(|prefix| line.trim_start().starts_with(prefix))
        })
        && lines.iter().all(|line| is_standalone_shell_command(line))
}

fn is_standalone_shell_command(line: &str) -> bool {
    let mut tokens = line.split_whitespace().peekable();
    while tokens.peek().is_some_and(|token| {
        token.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        })
    }) {
        tokens.next();
    }
    while tokens
        .peek()
        .is_some_and(|token| ["builtin", "command", "env", "sudo", "time"].contains(token))
    {
        tokens.next();
    }
    let Some(command) = tokens.next() else {
        return false;
    };
    ["./", "../", "/", "scripts/"]
        .iter()
        .any(|prefix| command.starts_with(prefix))
        || [
            "awk", "brew", "bun", "bundle", "cargo", "cat", "cd", "chmod", "cmake", "cp", "curl",
            "deno", "docker", "find", "gh", "git", "go", "grep", "jq", "kubectl", "ls", "make",
            "mkdir", "mv", "node", "npm", "npx", "pip", "pnpm", "python", "rg", "rsync", "ruby",
            "sed", "ssh", "swift", "tar", "touch", "wget", "yarn",
        ]
        .contains(&command)
}

fn strip_prompt_prefixes(input: &str) -> Option<String> {
    let lines = input.split('\n').collect::<Vec<_>>();
    let nonempty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if nonempty == 0 {
        return None;
    }
    let matches = lines
        .iter()
        .filter(|line| prompt_content(line).is_some())
        .count();
    if lines.len() <= 2 {
        if matches != nonempty {
            return None;
        }
    } else if matches <= nonempty / 2 {
        return None;
    }
    Some(
        lines
            .into_iter()
            .map(|line| prompt_content(line).unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn repair_wrapped_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let lowered = trimmed.to_ascii_lowercase();
    if !(lowered.starts_with("http://") || lowered.starts_with("https://"))
        || lowered.matches("http://").count() + lowered.matches("https://").count() != 1
    {
        return None;
    }
    let lines = trimmed.lines().collect::<Vec<_>>();
    if lines.len() < 2
        || lines
            .iter()
            .any(|line| line.trim().chars().any(char::is_whitespace))
    {
        return None;
    }
    let collapsed = lines.iter().map(|line| line.trim()).collect::<String>();
    collapsed.contains('.').then_some(collapsed)
}

fn strip_tracking_parameters(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.contains('\n')
        || !(trimmed.starts_with("http://") || trimmed.starts_with("https://"))
    {
        return None;
    }
    let (before_fragment, fragment) = trimmed
        .split_once('#')
        .map_or((trimmed, None), |(before, fragment)| {
            (before, Some(fragment))
        });
    let (base, query) = before_fragment.split_once('?')?;
    let host = base
        .split_once("://")?
        .1
        .split(['/', ':'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tracking = [
        "fbclid",
        "gclid",
        "gclsrc",
        "dclid",
        "msclkid",
        "mc_cid",
        "mc_eid",
        "igshid",
        "icid",
        "yclid",
        "twclid",
        "ttclid",
        "s_kwcid",
        "sc_cid",
        "_hsenc",
        "_hsmi",
        "vero_id",
        "wickedid",
        "oly_anon_id",
        "oly_enc_id",
        "rb_clickid",
        "spm",
        "ref_src",
        "ref_url",
    ];
    let youtube = host == "youtube.com" || host.ends_with(".youtube.com");
    let youtube_short = host == "youtu.be" || host.ends_with(".youtu.be");
    let kept = query
        .split('&')
        .filter(|item| {
            let name = item
                .split_once('=')
                .map_or(*item, |(name, _)| name)
                .to_ascii_lowercase();
            let host_tracking = (youtube
                && ["si", "feature", "pp", "ab_channel"].contains(&name.as_str()))
                || (youtube_short && ["si", "feature"].contains(&name.as_str()));
            !(name.starts_with("utm_") || tracking.contains(&name.as_str()) || host_tracking)
        })
        .collect::<Vec<_>>();
    if kept.len() == query.split('&').count() {
        return None;
    }
    let mut output = base.to_owned();
    if !kept.is_empty() {
        output.push('?');
        output.push_str(&kept.join("&"));
    }
    if let Some(fragment) = fragment {
        output.push('#');
        output.push_str(fragment);
    }
    Some(output)
}

fn quote_path_with_spaces(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.contains('\n')
        || !trimmed.contains(' ')
        || trimmed.contains("://")
        || trimmed.contains(['"', '\''])
    {
        return None;
    }
    let first = trimmed.split_whitespace().next()?;
    let explicit = first.starts_with('/')
        || first.starts_with("~/")
        || first.starts_with("./")
        || first.starts_with("../");
    let relative = first.matches('/').count() >= 2;
    if !(explicit || relative) || trimmed.split_whitespace().any(|word| word.starts_with('-')) {
        return None;
    }
    let tail = trimmed.rsplit_once('/').map_or(trimmed, |(_, tail)| tail);
    let words = tail.split_whitespace().collect::<Vec<_>>();
    if words.len() >= 3
        && words.iter().all(|word| {
            word.chars()
                .all(|character| character.is_ascii_lowercase() || character == '\'')
        })
    {
        return None;
    }
    Some(format!("\"{}\"", trimmed.replace('"', "\\\"")))
}

fn transform_multi_line_command(
    input: &str,
    options: CleanCopyOptions,
    padded_short_rows: bool,
) -> Option<String> {
    if !input.contains('\n') {
        return None;
    }
    let lines = input.lines().collect::<Vec<_>>();
    if !(2..=10).contains(&lines.len()) {
        return None;
    }
    let nonempty = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let explicit = has_explicit_command_join(input);
    if !explicit && is_unjoined_terminal_output(&nonempty) {
        return None;
    }
    if padded_short_rows && !explicit {
        return None;
    }
    let aggressiveness = options.command_flatten_aggressiveness;
    if aggressiveness != CommandFlattenAggressiveness::High && lines.len() > 4 {
        return None;
    }
    if nonempty.len() >= 2
        && nonempty
            .iter()
            .all(|line| is_standalone_shell_command(line))
    {
        return None;
    }
    let command_line_count = nonempty
        .iter()
        .filter(|line| is_likely_command_line(line))
        .count();
    if aggressiveness != CommandFlattenAggressiveness::High
        && !explicit
        && command_line_count == nonempty.len()
        && nonempty.len() >= 2
    {
        return None;
    }
    if aggressiveness != CommandFlattenAggressiveness::High && is_likely_command_list(&nonempty) {
        return None;
    }
    let strong = explicit
        || input.contains("&&")
        || input.contains("||")
        || input
            .lines()
            .any(|line| line.trim_start().starts_with("$ "))
        || contains_path_token(input);
    let known_prefix = nonempty.iter().any(|line| has_known_command_prefix(line));
    if aggressiveness != CommandFlattenAggressiveness::High
        && !strong
        && !known_prefix
        && !has_command_punctuation(input)
    {
        return None;
    }
    if aggressiveness != CommandFlattenAggressiveness::High
        && is_likely_source_code(input)
        && !strong
    {
        return None;
    }
    let mut score = usize::from(input.contains("\\\n"));
    score += usize::from(input.contains('|') || input.contains('&'));
    score += usize::from(
        input
            .lines()
            .any(|line| line.trim_start().starts_with("$ ")),
    );
    score += usize::from(is_single_command_with_indented_continuations(&nonempty));
    score += usize::from(
        nonempty.iter().any(|line| {
            line.trim_start().chars().next().is_some_and(|character| {
                character.is_ascii_alphanumeric() || "./~_-".contains(character)
            })
        }) || contains_path_token(input),
    );
    if score < aggressiveness.score_threshold() {
        return None;
    }
    let flattened = flatten_command_text(input, options.preserve_blank_lines_when_flattening);
    (flattened != input).then_some(flattened)
}

fn flatten_command_text(input: &str, preserve_blank_lines: bool) -> String {
    let mut output = String::new();
    let mut blank = false;
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            blank = true;
            continue;
        }
        if !output.is_empty() {
            if preserve_blank_lines && blank {
                output.push_str("\n\n");
            } else {
                output.push(' ');
            }
        }
        output.push_str(line.trim_end_matches('\\').trim_end());
        blank = false;
    }
    output
}

fn is_environment_assignment(line: &str) -> bool {
    line.split_whitespace().next().is_some_and(|token| {
        token.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|character| character == '_' || character.is_ascii_alphanumeric())
        })
    })
}

fn is_likely_command_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && (trimmed.starts_with("[[")
            || (!trimmed.ends_with('.')
                && trimmed.split_whitespace().next().is_some_and(|token| {
                    token.chars().all(|character| {
                        character.is_ascii_alphanumeric() || "./~_-".contains(character)
                    })
                })))
}

fn has_known_command_prefix(line: &str) -> bool {
    let first = line.split_whitespace().next().unwrap_or_default();
    is_standalone_shell_command(first)
        || [
            "sudo",
            "apt",
            "export",
            "open",
            "java",
            "perl",
            "bash",
            "zsh",
            "fish",
            "pwsh",
            "sh",
            "exit",
            "systemctl",
            "podman",
            "aws",
            "gcloud",
            "az",
            "xcodebuild",
        ]
        .iter()
        .any(|prefix| first.to_ascii_lowercase().starts_with(prefix))
}

fn contains_path_token(input: &str) -> bool {
    input.split_whitespace().any(|token| {
        let token = token.trim_matches(|character: char| {
            character.is_ascii_punctuation()
                && character != '/'
                && character != '.'
                && character != '~'
                && character != '_'
        });
        token.split_once('/').is_some_and(|(left, right)| {
            !left.is_empty()
                && !right.is_empty()
                && left.chars().all(is_path_token_character)
                && right.chars().all(is_path_token_character)
        })
    })
}

fn is_path_token_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || ".~_-".contains(character)
}

fn has_command_punctuation(input: &str) -> bool {
    input.contains('@')
        || input.contains('<')
        || input.contains('>')
        || input.split_whitespace().any(|token| {
            (token.starts_with("--") && token.len() > 2)
                || (token.starts_with('-')
                    && token.len() == 2
                    && token.as_bytes()[1].is_ascii_alphabetic())
                || is_environment_assignment(token)
                || token.starts_with("./")
                || token.starts_with("~/")
                || token.starts_with('/')
                || (token.starts_with('.') && token.len() > 1)
        })
}

fn is_single_command_with_indented_continuations(lines: &[&str]) -> bool {
    if lines.len() < 2 || !is_likely_command_line(lines[0]) {
        return false;
    }
    let mut saw_indented = false;
    for line in &lines[1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            saw_indented = true;
        } else if !["|", "&&", "||", ";", ">", "2>", "<", "--", "-"]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
        {
            return false;
        }
    }
    saw_indented
}

fn is_likely_command_list(lines: &[&str]) -> bool {
    let listish = lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            is_list_item(trimmed)
                || (!trimmed.contains(char::is_whitespace)
                    && trimmed.len() >= 4
                    && trimmed.chars().all(char::is_alphanumeric)
                    && !trimmed.contains(['.', '/', '$']))
        })
        .count();
    listish > lines.len() / 2
}

fn has_explicit_command_join(input: &str) -> bool {
    input.contains("\\\n")
        || input.lines().any(|line| {
            let trimmed = line.trim_end();
            trimmed.ends_with(['|', '&', ';']) || trimmed.ends_with("||")
        })
        || input.lines().skip(1).any(|line| {
            let trimmed = line.trim_start();
            ["| ", "& ", "&& ", "|| "]
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
        })
}

fn is_unjoined_terminal_output(lines: &[&str]) -> bool {
    let mixed_prompt_output = lines.iter().any(|line| line.trim_start().starts_with("$ "))
        && lines.iter().any(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("$ ") && !trimmed.starts_with(['|', '&', ';', '<', '>'])
        });
    let git_status = lines.iter().any(|line| {
        let trimmed = line.trim_start();
        ["?? ", "M ", "A ", "D "]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
    });
    mixed_prompt_output || git_status
}

fn dedent_common_prefix(input: &str) -> String {
    let lines = input.split('\n').collect::<Vec<_>>();
    let minimum = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.bytes()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count()
        })
        .min()
        .unwrap_or(0);
    if minimum == 0 {
        return input.to_owned();
    }
    lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                line
            } else {
                &line[minimum..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[must_use]
pub fn reformat_markdown(input: &str) -> String {
    let mut output = Vec::new();
    let mut paragraph = Vec::new();
    let mut in_fence = false;
    let flush = |output: &mut Vec<String>, paragraph: &mut Vec<String>| {
        if !paragraph.is_empty() {
            output.push(paragraph.join(" "));
            paragraph.clear();
        }
    };
    for line in input.split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            flush(&mut output, &mut paragraph);
            in_fence = !in_fence;
            output.push(line.to_owned());
        } else if in_fence {
            output.push(line.to_owned());
        } else if trimmed.is_empty() {
            flush(&mut output, &mut paragraph);
            output.push(String::new());
        } else if is_markdown_structure(trimmed) {
            flush(&mut output, &mut paragraph);
            output.push(line.to_owned());
        } else {
            paragraph.push(trimmed.to_owned());
        }
    }
    flush(&mut output, &mut paragraph);
    output.join("\n")
}

#[must_use]
pub fn is_likely_markdown(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains("```") {
        return true;
    }
    let mut headings = 0;
    let mut lists = 0;
    for line in trimmed.lines().map(str::trim) {
        if line
            .strip_prefix('#')
            .is_some_and(|tail| tail.starts_with('#') || tail.starts_with(' '))
        {
            headings += 1;
        }
        let bullet = ["- ", "* ", "+ "]
            .iter()
            .any(|prefix| line.starts_with(prefix));
        let numbered = line.split_once(['.', ')']).is_some_and(|(number, rest)| {
            !number.is_empty()
                && number.chars().all(|character| character.is_ascii_digit())
                && rest.starts_with(' ')
        });
        if bullet || numbered {
            lists += 1;
        }
    }
    headings >= 1 || lists >= 2
}

fn is_markdown_structure(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with('>')
        || line.starts_with('|')
        || ["- ", "* ", "+ "]
            .iter()
            .any(|prefix| line.starts_with(prefix))
        || line.split_once(['.', ')']).is_some_and(|(number, rest)| {
            !number.is_empty()
                && number.chars().all(|ch| ch.is_ascii_digit())
                && rest.starts_with(' ')
        })
}

#[cfg(test)]
mod classifier_tests {
    use super::*;

    #[test]
    fn slash_command_classifier_rejects_each_invalid_boundary() {
        assert_eq!(
            strip_slash_command_decoration("/task:run first\n second"),
            Some("/task:run first second".to_owned())
        );
        for input in [
            "/:run first\nsecond",
            "/run: first\nsecond",
            "/run.bad first\nsecond",
            "/one:two:three first\nsecond",
            "\"/run no escaped quote\"",
            "\"/run \\\"unterminated",
            "/run \\\"not outer quoted\\\"",
        ] {
            assert_eq!(strip_slash_command_decoration(input), None);
        }
    }

    #[test]
    fn separated_bullet_classifier_requires_multiple_valid_blocks() {
        let no_width = WrapWidthEvidence { width: None };
        assert_eq!(reflow_separated_bullets(&["• one"], no_width), None);
        assert_eq!(
            reflow_separated_bullets(&["• one", "", "• two"], no_width),
            Some("• one\n\n• two".to_owned())
        );
        assert_eq!(
            reflow_separated_bullets(&["• one", "", "continuation", "• two"], no_width,),
            None
        );
        assert_eq!(
            reflow_separated_bullets(&["• one", "• extra", "", "• two"], no_width),
            None
        );
    }

    #[test]
    fn primitive_classifiers_pin_source_boundaries() {
        for item in ["- item", "* item", "• item", "1. item", "2) item"] {
            assert!(is_list_item(item));
        }
        for item in [". item", "x. item", "1.item", "", "plain"] {
            assert!(!is_list_item(item));
        }
        assert!(is_likely_source_code("def work\nbegin\nend"));
        assert!(is_likely_source_code("fn work() {\n}"));
        assert!(!is_likely_source_code("ordinary begin prose"));

        assert!(is_environment_assignment("NAME=value command"));
        assert!(is_environment_assignment("_NAME=value command"));
        for line in ["=value command", "BAD-NAME=value command", "plain"] {
            assert!(!is_environment_assignment(line));
        }
        for line in ["cargo test", "sudo git status", "./script run", "/bin/sh"] {
            assert!(is_standalone_shell_command(line));
        }
        for line in ["", "unknown command", "BAD-NAME=value cargo test"] {
            assert!(!is_standalone_shell_command(line));
        }
        assert!(is_compact_shell_block(&["cargo test", "pnpm install"]));
        assert!(!is_compact_shell_block(&["cargo test"]));
        assert!(!is_compact_shell_block(&["cargo test", "--flag"]));
        assert!(!is_compact_shell_block(&["cargo test", "unknown command"]));
    }

    #[test]
    fn command_signal_classifiers_are_independent() {
        for line in ["git run", "sudo run", "xcodebuild test", "./script"] {
            assert!(has_known_command_prefix(line));
        }
        assert!(!has_known_command_prefix("unknown run"));

        for text in ["src/main.rs next", "a/b", "feature/login-flow"] {
            assert!(contains_path_token(text));
        }
        for text in ["/root", "left/", "/right", "a/b@invalid", "plain"] {
            assert!(!contains_path_token(text));
        }
        assert!(is_path_token_character('a'));
        assert!(is_path_token_character('_'));
        assert!(!is_path_token_character('/'));

        for text in [
            "user@example.com",
            "custom --flag",
            "custom -f value",
            "NAME=value custom",
            "./script",
            "~/script",
            "/script",
            ".hidden",
            "custom < input",
            "custom > output",
        ] {
            assert!(has_command_punctuation(text), "{text}");
        }
        for text in [
            "custom - value",
            "custom -- value",
            "custom . value",
            "plain",
        ] {
            assert!(!has_command_punctuation(text), "{text}");
        }
    }

    #[test]
    fn command_shape_classifiers_cover_lengths_and_continuations() {
        assert!(is_single_command_with_indented_continuations(&[
            "custom run",
            "  continuation"
        ]));
        assert!(!is_single_command_with_indented_continuations(&[
            "custom run",
            "--flag"
        ]));
        assert!(is_single_command_with_indented_continuations(&[
            "custom run",
            "  continuation",
            "--flag"
        ]));
        assert!(!is_single_command_with_indented_continuations(&[
            "custom run"
        ]));
        assert!(!is_single_command_with_indented_continuations(&[
            "not a sentence.",
            "  continuation"
        ]));
        assert!(!is_single_command_with_indented_continuations(&[
            "custom run",
            "another command"
        ]));

        assert!(is_likely_command_list(&["alpha", "bravo", "src/main.rs"]));
        assert!(is_likely_command_list(&["- one", "plain", "* two"]));
        for lines in [
            &["abc", "def"] as &[&str],
            &["src/main.rs", "feature/branch"],
            &["$value", "plain"],
        ] {
            assert!(!is_likely_command_list(lines));
        }

        assert!(is_unjoined_terminal_output(&["$ command", "output"]));
        assert!(is_unjoined_terminal_output(&["g", " M file"]));
        assert!(!is_unjoined_terminal_output(&["$ one", "$ two"]));
        assert!(!is_unjoined_terminal_output(&["custom", "| next"]));
    }

    #[test]
    fn prose_reflow_requires_a_blockquote_majority() {
        let input = concat!(
            "> quoted line that is deliberately longer than sixty characters for reflow\n",
            "second ordinary line that remains part of the same wrapped paragraph\n",
            "third ordinary line that remains part of the same wrapped paragraph\n",
            "fourth ordinary line that remains part of the same wrapped paragraph"
        );
        assert_eq!(
            reflow_plain_prose(input, false, WrapWidthEvidence { width: None }),
            Some(input.replace('\n', " "))
        );
    }

    #[test]
    fn command_transform_guards_and_scores_are_independently_observable() {
        let normal = CleanCopyOptions::default();
        let low = CleanCopyOptions {
            command_flatten_aggressiveness: CommandFlattenAggressiveness::Low,
            ..Default::default()
        };
        let high = CleanCopyOptions {
            command_flatten_aggressiveness: CommandFlattenAggressiveness::High,
            ..Default::default()
        };

        assert_eq!(
            transform_multi_line_command("alpha\nbravo", normal, false),
            None
        );
        assert_eq!(
            transform_multi_line_command("alpha\nbravo", high, false),
            Some("alpha bravo".to_owned())
        );
        assert_eq!(
            transform_multi_line_command("git status\npnpm install", high, false),
            None
        );
        assert_eq!(
            transform_multi_line_command("custom run\n  continuation", normal, false),
            None
        );
        assert_eq!(
            transform_multi_line_command("custom run\n  continuation", high, false),
            Some("custom run continuation".to_owned())
        );

        let source = "fn something(\n  --flag";
        assert_eq!(transform_multi_line_command(source, normal, false), None);
        assert_eq!(
            transform_multi_line_command(source, high, false),
            Some("fn something( --flag".to_owned())
        );
        let strong_source = "fn something( \\\n  --flag";
        assert_eq!(
            transform_multi_line_command(strong_source, normal, false),
            Some("fn something( --flag".to_owned())
        );

        for operator in ['|', '&'] {
            let input = format!("custom --flag {operator} value\n  continuation.");
            let lines = input.lines().collect::<Vec<_>>();
            assert!(is_single_command_with_indented_continuations(&lines));
            assert!(input.contains('|') || input.contains('&'));
            assert!(lines.iter().any(|line| line.starts_with('c')));
            assert!(!is_unjoined_terminal_output(&lines));
            assert!(!lines.iter().all(|line| is_standalone_shell_command(line)));
            assert!(!is_likely_command_list(&lines));
            assert!(!is_likely_source_code(&input));
            assert_eq!(
                transform_multi_line_command(&input, low, false),
                Some(format!("custom --flag {operator} value continuation."))
            );
        }
        assert_eq!(
            transform_multi_line_command("$ custom | value \\\n  continuation.", low, false),
            Some("$ custom | value continuation.".to_owned())
        );
    }

    #[test]
    fn command_flattening_blank_policy_is_explicit() {
        assert_eq!(flatten_command_text("cmd\n\nnext", true), "cmd\n\nnext");
        assert_eq!(flatten_command_text("cmd\n\nnext", false), "cmd next");
    }

    #[test]
    fn path_token_edge_punctuation_preserves_path_syntax() {
        assert!(contains_path_token("!a/b!"));
        assert!(!contains_path_token("/a/b/"));
        assert!(contains_path_token("./ab."));
        assert!(contains_path_token("~/ab~"));
        assert!(contains_path_token("_/ab_"));
    }

    #[test]
    fn markdown_classifiers_require_complete_numbered_items() {
        for line in ["# heading", "> quote", "| table", "- item", "1. item"] {
            assert!(is_markdown_structure(line));
        }
        for line in ["plain", ". item", "x. item", "1.item"] {
            assert!(!is_markdown_structure(line));
        }
        assert!(is_likely_markdown("1. one\n2) two"));
        assert!(!is_likely_markdown(". one\nx. two"));
    }
}
