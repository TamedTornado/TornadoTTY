#![forbid(unsafe_code)]

use std::fs::{File, OpenOptions, remove_file};
use std::io::Write;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wayland_client::protocol::{wl_keyboard, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

const EVDEV_Y: u32 = 21;
const EVDEV_ENTER: u32 = 28;
const EVDEV_LEFT_SHIFT: u32 = 42;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Action {
    #[default]
    Tap,
    Shifted,
    Hold,
    Enter,
}

impl Action {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "tap" => Ok(Self::Tap),
            "shifted" => Ok(Self::Shifted),
            "hold" => Ok(Self::Hold),
            "enter" => Ok(Self::Enter),
            _ => Err(format!("unsupported action: {value}")),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Tap => "tap",
            Self::Shifted => "shifted",
            Self::Hold => "hold",
            Self::Enter => "enter",
        }
    }

    const fn sequence(self) -> &'static str {
        match self {
            Self::Tap => "key-down,key-up,enter-down,enter-up",
            Self::Shifted => "modifier-down,key-down,key-up,modifier-up,enter-down,enter-up",
            Self::Hold => "key-down,bounded-hold,key-up,enter-down,enter-up",
            Self::Enter => "enter-down,enter-up",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Map {
    Us,
    De,
    Remap,
}

impl Map {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "us" => Ok(Self::Us),
            "de" => Ok(Self::De),
            "remap" => Ok(Self::Remap),
            _ => Err(format!("unsupported map: {value}")),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Us => "us",
            Self::De => "de",
            Self::Remap => "remap",
        }
    }

    const fn expected(self) -> &'static str {
        match self {
            Self::Us => "y",
            Self::De => "z",
            Self::Remap => "ü",
        }
    }

    fn keymap(self) -> String {
        let symbol = match self {
            Self::Us => "y, Y",
            Self::De => "z, Z",
            Self::Remap => "udiaeresis, Udiaeresis",
        };
        format!(
            r#"xkb_keymap {{
xkb_keycodes "zentty" {{ include "evdev+aliases(qwerty)" }};
xkb_types "zentty" {{ include "complete" }};
xkb_compatibility "zentty" {{ include "complete" }};
xkb_symbols "zentty" {{
    key <AD06> {{ [ {symbol} ] }};
    key <LFSH> {{ [ Shift_L ] }};
    key <RTRN> {{ [ Return ] }};
}};
}};
"#
        )
    }
}

struct Arguments {
    map: Map,
    keycode: u32,
    action: Action,
    hold_ms: Option<u64>,
    receipt: PathBuf,
}

impl Arguments {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut map = None;
        let mut keycode = None;
        let mut action = Action::default();
        let mut hold_ms = None;
        let mut receipt = None;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--map" => {
                    map = Some(Map::parse(
                        &arguments.next().ok_or("--map requires a value")?,
                    )?);
                }
                "--keycode" => {
                    let value = arguments.next().ok_or("--keycode requires a value")?;
                    keycode = Some(
                        value
                            .parse::<u32>()
                            .map_err(|_| "--keycode must be an unsigned integer")?,
                    );
                }
                "--receipt" => {
                    receipt = Some(PathBuf::from(
                        arguments.next().ok_or("--receipt requires a value")?,
                    ));
                }
                "--action" => {
                    action = Action::parse(&arguments.next().ok_or("--action requires a value")?)?;
                }
                "--hold-ms" => {
                    let value = arguments.next().ok_or("--hold-ms requires a value")?;
                    hold_ms = Some(
                        value
                            .parse::<u64>()
                            .map_err(|_| "--hold-ms must be an unsigned integer")?,
                    );
                }
                _ => return Err(format!("unexpected argument: {argument}")),
            }
        }
        let receipt = receipt.ok_or("--receipt is required")?;
        if !receipt.is_absolute() || receipt.exists() {
            return Err("--receipt must be one absent absolute path".to_owned());
        }
        let keycode = keycode.ok_or("--keycode is required")?;
        if keycode != EVDEV_Y {
            return Err(format!(
                "only reviewed physical keycode {EVDEV_Y} is accepted"
            ));
        }
        match (action, hold_ms) {
            (Action::Hold, Some(250..=2_000)) => {}
            (Action::Hold, _) => {
                return Err("hold action requires --hold-ms between 250 and 2000".to_owned());
            }
            (_, None) => {}
            (_, Some(_)) => return Err("--hold-ms is valid only for hold action".to_owned()),
        }
        Ok(Self {
            map: map.ok_or("--map is required")?,
            keycode,
            action,
            hold_ms,
            receipt,
        })
    }
}

#[derive(Default)]
struct WaylandState {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpVirtualKeyboardManagerV1>,
    repeat: Option<(i32, i32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == wl_seat::WlSeat::interface().name {
                state.seat = Some(registry.bind(name, version.min(7), queue, ()));
            } else if interface == ZwpVirtualKeyboardManagerV1::interface().name {
                state.manager = Some(registry.bind(name, version.min(1), queue, ()));
            }
        }
    }
}

delegate_noop!(WaylandState: ignore wl_seat::WlSeat);
delegate_noop!(WaylandState: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(WaylandState: ignore ZwpVirtualKeyboardV1);

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::RepeatInfo { rate, delay } = event {
            state.repeat = Some((rate, delay));
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("wayland-keycode-driver: error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let keymap = arguments.map.keymap();
    let mut keymap_bytes = keymap.into_bytes();
    keymap_bytes.push(0);
    let (keymap_file, keymap_path) = temporary_keymap(&keymap_bytes)?;
    let result = send_keycode(&arguments, &keymap_file, keymap_bytes.len());
    drop(keymap_file);
    let _ = remove_file(keymap_path);
    let (repeat_rate, repeat_delay_ms) = result?;
    std::fs::write(
        &arguments.receipt,
        format!(
            "map={} keycode={} expected={} action={} sequence={} hold_ms={} repeat_rate={} repeat_delay_ms={} keymap_bytes={}\n",
            arguments.map.name(),
            arguments.keycode,
            match arguments.action {
                Action::Shifted => "Y",
                Action::Hold => "repeat",
                Action::Enter => "newline",
                Action::Tap => arguments.map.expected(),
            },
            arguments.action.name(),
            arguments.action.sequence(),
            arguments.hold_ms.unwrap_or(0),
            repeat_rate,
            repeat_delay_ms,
            keymap_bytes.len()
        ),
    )
    .map_err(|error| format!("could not write receipt: {error}"))?;
    Ok(())
}

fn temporary_keymap(contents: &[u8]) -> Result<(File, PathBuf), String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock is before Unix epoch: {error}"))?
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("zentty-keymap-{}-{nonce}.xkb", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("could not create private keymap: {error}"))?;
    file.write_all(contents)
        .and_then(|()| file.flush())
        .map_err(|error| format!("could not write private keymap: {error}"))?;
    Ok((file, path))
}

fn send_keycode(arguments: &Arguments, keymap: &File, size: usize) -> Result<(i32, i32), String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("could not connect to controlled Wayland display: {error}"))?;
    let mut event_queue = connection.new_event_queue();
    let queue = event_queue.handle();
    connection.display().get_registry(&queue, ());
    let mut state = WaylandState::default();
    event_queue
        .roundtrip(&mut state)
        .map_err(|error| format!("could not enumerate Wayland globals: {error}"))?;
    let seat = state
        .seat
        .as_ref()
        .ok_or("controlled compositor has no seat")?
        .clone();
    let _observed_keyboard = seat.get_keyboard(&queue, ());
    event_queue
        .roundtrip(&mut state)
        .map_err(|error| format!("could not observe compositor repeat configuration: {error}"))?;
    let repeat = state.repeat.unwrap_or((0, 0));
    if arguments.action == Action::Hold && (repeat.0 <= 0 || repeat.1 <= 0) {
        return Err(
            "controlled compositor did not publish positive repeat configuration".to_owned(),
        );
    }
    let manager = state
        .manager
        .as_ref()
        .ok_or("controlled compositor has no virtual-keyboard manager")?;
    let keyboard = manager.create_virtual_keyboard(&seat, &queue, ());
    let size = u32::try_from(size).map_err(|_| "keymap is too large")?;
    keyboard.keymap(1, keymap.as_fd(), size);
    let time = u32::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock is before Unix epoch: {error}"))?
            .as_millis()
            % u128::from(u32::MAX),
    )
    .map_err(|_| "timestamp conversion failed")?;
    match arguments.action {
        Action::Tap => {
            tap(&keyboard, time, arguments.keycode);
            tap(&keyboard, time.saturating_add(2), EVDEV_ENTER);
        }
        Action::Shifted => {
            keyboard.key(time, EVDEV_LEFT_SHIFT, 1);
            keyboard.modifiers(1, 0, 0, 0);
            tap(&keyboard, time.saturating_add(1), arguments.keycode);
            keyboard.key(time.saturating_add(3), EVDEV_LEFT_SHIFT, 0);
            keyboard.modifiers(0, 0, 0, 0);
            tap(&keyboard, time.saturating_add(4), EVDEV_ENTER);
        }
        Action::Hold => {
            keyboard.key(time, arguments.keycode, 1);
            connection
                .flush()
                .map_err(|error| format!("could not flush held key-down: {error}"))?;
            sleep(Duration::from_millis(arguments.hold_ms.unwrap_or_default()));
            keyboard.key(time.saturating_add(1), arguments.keycode, 0);
            tap(&keyboard, time.saturating_add(2), EVDEV_ENTER);
        }
        Action::Enter => tap(&keyboard, time, EVDEV_ENTER),
    }
    connection
        .flush()
        .map_err(|error| format!("could not flush physical key events: {error}"))?;
    event_queue
        .roundtrip(&mut state)
        .map_err(|error| format!("compositor rejected physical key events: {error}"))?;
    Ok(repeat)
}

fn tap(keyboard: &ZwpVirtualKeyboardV1, time: u32, keycode: u32) {
    keyboard.key(time, keycode, 1);
    keyboard.key(time.saturating_add(1), keycode, 0);
}

#[cfg(test)]
mod tests {
    use super::{Action, Arguments, EVDEV_Y, Map};

    #[test]
    fn maps_keep_one_physical_position_and_distinct_results() {
        assert_eq!(EVDEV_Y, 21);
        assert_eq!(Map::Us.expected(), "y");
        assert_eq!(Map::De.expected(), "z");
        assert_eq!(Map::Remap.expected(), "ü");
        for map in [Map::Us, Map::De, Map::Remap] {
            let keymap = map.keymap();
            assert!(keymap.contains("key <AD06>"));
            assert!(keymap.contains("key <RTRN>"));
        }
    }

    #[test]
    fn arguments_reject_symbol_or_unreviewed_keycode_injection() {
        let root = std::env::temp_dir().join("zentty-wayland-keycode-test-receipt");
        let symbol = Arguments::parse(
            [
                "--map",
                "us",
                "--key",
                "y",
                "--receipt",
                &root.to_string_lossy(),
            ]
            .into_iter()
            .map(str::to_owned),
        );
        assert!(symbol.is_err());
        let wrong_code = Arguments::parse(
            [
                "--map",
                "us",
                "--keycode",
                "22",
                "--receipt",
                &root.to_string_lossy(),
            ]
            .into_iter()
            .map(str::to_owned),
        );
        assert!(wrong_code.is_err());
    }

    #[test]
    fn actions_have_reviewed_order_and_bounded_holds() {
        assert_eq!(
            Action::Shifted.sequence(),
            "modifier-down,key-down,key-up,modifier-up,enter-down,enter-up"
        );
        assert_eq!(
            Action::Hold.sequence(),
            "key-down,bounded-hold,key-up,enter-down,enter-up"
        );
        let root = std::env::temp_dir().join("zentty-wayland-hold-test-receipt");
        for hold in ["0", "249", "2001"] {
            let result = Arguments::parse(
                [
                    "--map",
                    "us",
                    "--keycode",
                    "21",
                    "--action",
                    "hold",
                    "--hold-ms",
                    hold,
                    "--receipt",
                    &root.to_string_lossy(),
                ]
                .into_iter()
                .map(str::to_owned),
            );
            assert!(result.is_err());
        }
    }
}
