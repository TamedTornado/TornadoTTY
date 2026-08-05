use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedArguments {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
    positionals: Vec<String>,
}

impl ParsedArguments {
    #[must_use]
    pub fn parse(
        arguments: &[String],
        value_options: &[String],
        boolean_options: &[String],
    ) -> Self {
        let value_options: BTreeSet<&str> = value_options.iter().map(String::as_str).collect();
        let boolean_options: BTreeSet<&str> = boolean_options.iter().map(String::as_str).collect();
        let mut values = BTreeMap::new();
        let mut flags = BTreeSet::new();
        let mut positionals = Vec::new();
        let mut arguments = arguments.iter();
        while let Some(argument) = arguments.next() {
            if value_options.contains(argument.as_str()) {
                if let Some(value) = arguments.next() {
                    values.insert(argument.clone(), value.clone());
                }
            } else if boolean_options.contains(argument.as_str()) {
                flags.insert(argument.clone());
            } else if let Some(cluster) = parse_cluster(argument, &value_options, &boolean_options)
            {
                match cluster {
                    Cluster::Flags(clustered) => flags.extend(clustered),
                    Cluster::Value(option, value) => {
                        values.insert(option, value);
                    }
                }
            } else {
                positionals.push(argument.clone());
            }
        }
        Self {
            values,
            flags,
            positionals,
        }
    }

    #[must_use]
    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    #[must_use]
    pub fn flags(&self) -> Vec<String> {
        self.flags.iter().cloned().collect()
    }

    #[must_use]
    pub fn positionals(&self) -> &[String] {
        &self.positionals
    }

    #[must_use]
    pub fn value(&self, option: &str) -> Option<&str> {
        self.values.get(option).map(String::as_str)
    }

    #[must_use]
    pub fn has_flag(&self, option: &str) -> bool {
        self.flags.contains(option)
    }
}

enum Cluster {
    Flags(Vec<String>),
    Value(String, String),
}

fn parse_cluster(
    argument: &str,
    value_options: &BTreeSet<&str>,
    boolean_options: &BTreeSet<&str>,
) -> Option<Cluster> {
    if !argument.starts_with('-') || argument.starts_with("--") || argument.chars().count() <= 2 {
        return None;
    }
    let clustered: Vec<String> = argument
        .chars()
        .skip(1)
        .map(|character| format!("-{character}"))
        .collect();
    if clustered
        .iter()
        .all(|option| boolean_options.contains(option.as_str()))
    {
        return Some(Cluster::Flags(clustered));
    }
    let option = clustered
        .iter()
        .find(|option| value_options.contains(option.as_str()))?;
    let value = argument.strip_prefix(option)?;
    Some(Cluster::Value(option.clone(), value.to_owned()))
}
