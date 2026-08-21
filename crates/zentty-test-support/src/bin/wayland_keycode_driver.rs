#![forbid(unsafe_code)]

use std::fs::{File, OpenOptions, remove_file};
use std::io::Write;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

const EVDEV_Y: u32 = 21;
const EVDEV_ENTER: u32 = 28;

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
    receipt: PathBuf,
}

impl Arguments {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut map = None;
        let mut keycode = None;
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
        Ok(Self {
            map: map.ok_or("--map is required")?,
            keycode,
            receipt,
        })
    }
}

#[derive(Default)]
struct WaylandState {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpVirtualKeyboardManagerV1>,
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
    result?;
    std::fs::write(
        &arguments.receipt,
        format!(
            "map={} keycode={} expected={} keymap_bytes={}\n",
            arguments.map.name(),
            arguments.keycode,
            arguments.map.expected(),
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

fn send_keycode(arguments: &Arguments, keymap: &File, size: usize) -> Result<(), String> {
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
        .ok_or("controlled compositor has no seat")?;
    let manager = state
        .manager
        .as_ref()
        .ok_or("controlled compositor has no virtual-keyboard manager")?;
    let keyboard = manager.create_virtual_keyboard(seat, &queue, ());
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
    keyboard.key(time, arguments.keycode, 1);
    keyboard.key(time.saturating_add(1), arguments.keycode, 0);
    keyboard.key(time.saturating_add(2), EVDEV_ENTER, 1);
    keyboard.key(time.saturating_add(3), EVDEV_ENTER, 0);
    connection
        .flush()
        .map_err(|error| format!("could not flush physical key events: {error}"))?;
    event_queue
        .roundtrip(&mut state)
        .map_err(|error| format!("compositor rejected physical key events: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Arguments, EVDEV_Y, Map};

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
}
