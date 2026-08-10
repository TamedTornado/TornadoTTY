#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshDestination {
    pub target: String,
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

impl SshDestination {
    #[must_use]
    pub fn new(target: &str, user: Option<&str>, host: &str, port: Option<u16>) -> Self {
        Self {
            target: target.to_owned(),
            user: user.map(str::to_owned),
            host: host.to_owned(),
            port,
        }
    }
}

#[must_use]
pub fn parse_ssh_destination(argv: &[&str]) -> Option<SshDestination> {
    if argv.len() <= 1 {
        return None;
    }
    let mut index = 1;
    let mut explicit_user = None;
    let mut port = None;
    let mut target = None;
    // The slice bounds the parser independently of option bookkeeping. A
    // malformed argv can therefore never turn an index mistake into an
    // unbounded scan.
    for _ in &argv[1..] {
        let token = *argv.get(index)?;
        if token == "--" {
            target = argv.get(index + 1).copied();
            break;
        }
        if token == "-l" || token == "-p" {
            let value = *argv.get(index + 1)?;
            if token == "-l" {
                explicit_user = nonempty(value);
            } else {
                port = Some(parse_port(value)?);
            }
            index += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix("-l").filter(|value| !value.is_empty()) {
            explicit_user = nonempty(value);
            index += 1;
            continue;
        }
        if let Some(value) = token.strip_prefix("-p").filter(|value| !value.is_empty()) {
            port = Some(parse_port(value)?);
            index += 1;
            continue;
        }
        if token.starts_with('-') {
            index += if option_consumes_value(token) { 2 } else { 1 };
            continue;
        }
        target = Some(token);
        break;
    }

    let target = nonempty(target?)?;
    let (target_user, host) = match target.split_once('@') {
        Some((user, host)) => (Some(nonempty(user)?), nonempty(host)?),
        None => (None, target),
    };
    let user = target_user.or(explicit_user);
    let display_target = user.map_or_else(|| host.to_owned(), |user| format!("{user}@{host}"));
    Some(SshDestination {
        target: display_target,
        user: user.map(str::to_owned),
        host: host.to_owned(),
        port,
    })
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn parse_port(value: &str) -> Option<u16> {
    let port = value.parse::<u16>().ok()?;
    (port != 0).then_some(port)
}

fn option_consumes_value(option: &str) -> bool {
    matches!(
        option,
        "-B" | "-b"
            | "-c"
            | "-D"
            | "-E"
            | "-e"
            | "-F"
            | "-I"
            | "-i"
            | "-J"
            | "-L"
            | "-m"
            | "-O"
            | "-o"
            | "-Q"
            | "-R"
            | "-S"
            | "-W"
            | "-w"
    )
}
