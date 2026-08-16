use std::collections::BTreeMap;

pub(super) struct ParsedSignalArguments {
    pub kind: String,
    pub positionals: Vec<String>,
    pub options: BTreeMap<String, String>,
}

pub(super) fn parse_signal_arguments(
    arguments: &[String],
    label: &str,
) -> Result<ParsedSignalArguments, (&'static str, String)> {
    let Some(kind) = arguments.first() else {
        return Err(("invalid_request", format!("{label} kind is missing")));
    };
    let mut positionals = Vec::new();
    let mut options = BTreeMap::new();
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if let Some(option) = argument.strip_prefix("--") {
            let Some(value) = arguments.get(index + 1) else {
                return Err((
                    "invalid_request",
                    format!("{label} option {argument} is missing its value"),
                ));
            };
            if options.insert(option.to_owned(), value.clone()).is_some() {
                return Err((
                    "invalid_request",
                    format!("duplicate {label} option {argument}"),
                ));
            }
            index += 2;
        } else {
            positionals.push(argument.clone());
            index += 1;
        }
    }
    Ok(ParsedSignalArguments {
        kind: kind.clone(),
        positionals,
        options,
    })
}

pub(super) fn validate_signal_options(
    options: &BTreeMap<String, String>,
    allowed: &[&str],
) -> Result<(), (&'static str, String)> {
    if let Some(option) = options
        .keys()
        .find(|option| !allowed.contains(&option.as_str()))
    {
        return Err((
            "invalid_request",
            format!("unsupported agent signal option --{option}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_signal_arguments;

    #[test]
    fn parser_preserves_positionals_and_hostile_option_values() {
        let parsed = parse_signal_arguments(
            &[
                "lifecycle".to_owned(),
                "running".to_owned(),
                "--text".to_owned(),
                "space and λ --literal".to_owned(),
            ],
            "agent signal",
        )
        .unwrap();
        assert_eq!(parsed.kind, "lifecycle");
        assert_eq!(parsed.positionals, ["running"]);
        assert_eq!(
            parsed.options.get("text").map(String::as_str),
            Some("space and λ --literal")
        );
    }

    #[test]
    fn parser_rejects_missing_and_duplicate_option_values() {
        assert!(
            parse_signal_arguments(
                &["lifecycle".to_owned(), "--text".to_owned()],
                "agent signal"
            )
            .is_err()
        );
        assert!(
            parse_signal_arguments(
                &[
                    "lifecycle".to_owned(),
                    "--text".to_owned(),
                    "one".to_owned(),
                    "--text".to_owned(),
                    "two".to_owned(),
                ],
                "agent signal"
            )
            .is_err()
        );
    }
}
