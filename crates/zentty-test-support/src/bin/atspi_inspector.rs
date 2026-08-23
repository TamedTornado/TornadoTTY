#![allow(unsafe_code)]

use serde_json::{Value, json};
use std::env;
use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

enum AtspiAccessible {}
enum AtspiAction {}
enum AtspiStateSet {}
enum GError {}

const STATE_ACTIVE: c_int = 1;
const STATE_ENABLED: c_int = 8;
const STATE_FOCUSABLE: c_int = 11;
const STATE_FOCUSED: c_int = 12;
const STATE_SELECTED: c_int = 23;
const STATE_SHOWING: c_int = 25;
const STATE_VISIBLE: c_int = 30;

#[link(name = "atspi")]
unsafe extern "C" {
    fn atspi_init() -> c_int;
    fn atspi_exit() -> c_int;
    fn atspi_get_desktop_count() -> c_int;
    fn atspi_get_desktop(index: c_int) -> *mut AtspiAccessible;
    fn atspi_accessible_get_name(
        object: *mut AtspiAccessible,
        error: *mut *mut GError,
    ) -> *mut c_char;
    fn atspi_accessible_get_description(
        object: *mut AtspiAccessible,
        error: *mut *mut GError,
    ) -> *mut c_char;
    fn atspi_accessible_get_role_name(
        object: *mut AtspiAccessible,
        error: *mut *mut GError,
    ) -> *mut c_char;
    fn atspi_accessible_get_child_count(
        object: *mut AtspiAccessible,
        error: *mut *mut GError,
    ) -> c_int;
    fn atspi_accessible_get_child_at_index(
        object: *mut AtspiAccessible,
        index: c_int,
        error: *mut *mut GError,
    ) -> *mut AtspiAccessible;
    fn atspi_accessible_get_process_id(
        object: *mut AtspiAccessible,
        error: *mut *mut GError,
    ) -> c_uint;
    fn atspi_accessible_get_state_set(object: *mut AtspiAccessible) -> *mut AtspiStateSet;
    fn atspi_state_set_contains(states: *mut AtspiStateSet, state: c_int) -> c_int;
    fn atspi_accessible_get_action_iface(object: *mut AtspiAccessible) -> *mut AtspiAction;
    fn atspi_action_get_n_actions(action: *mut AtspiAction, error: *mut *mut GError) -> c_int;
    fn atspi_action_get_action_name(
        action: *mut AtspiAction,
        index: c_int,
        error: *mut *mut GError,
    ) -> *mut c_char;
    fn atspi_action_do_action(
        action: *mut AtspiAction,
        index: c_int,
        error: *mut *mut GError,
    ) -> c_int;
}

#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_error_free(error: *mut GError);
    fn g_free(memory: *mut c_void);
}

#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_object_unref(object: *mut c_void);
}

struct OwnedAccessible(*mut AtspiAccessible);

impl Drop for OwnedAccessible {
    fn drop(&mut self) {
        // SAFETY: AT-SPI child accessors return an owned GObject reference.
        unsafe { g_object_unref(self.0.cast()) };
    }
}

struct OwnedObject(*mut c_void);

impl Drop for OwnedObject {
    fn drop(&mut self) {
        // SAFETY: This wrapper is created only for full-transfer GObjects.
        unsafe { g_object_unref(self.0) };
    }
}

fn take_string(pointer: *mut c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: AT-SPI returns a NUL-terminated GLib-owned string. Copy it before
    // releasing that allocation with the matching GLib allocator.
    let value = unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: The string was allocated by GLib for this caller.
    unsafe { g_free(pointer.cast()) };
    value
}

fn call_string(
    call: unsafe extern "C" fn(*mut AtspiAccessible, *mut *mut GError) -> *mut c_char,
    object: *mut AtspiAccessible,
) -> String {
    let mut error = ptr::null_mut();
    // SAFETY: `object` is a live registry-owned or owned accessible proxy.
    let value = unsafe { call(object, &raw mut error) };
    if !error.is_null() {
        // SAFETY: GLib returned this error to the caller.
        unsafe { g_error_free(error) };
        return String::new();
    }
    take_string(value)
}

fn child_count(object: *mut AtspiAccessible) -> c_int {
    let mut error = ptr::null_mut();
    // SAFETY: `object` is a live accessible proxy.
    let count = unsafe { atspi_accessible_get_child_count(object, &raw mut error) };
    if !error.is_null() {
        // SAFETY: GLib returned this error to the caller.
        unsafe { g_error_free(error) };
        return 0;
    }
    count.max(0)
}

fn process_id(object: *mut AtspiAccessible) -> u32 {
    let mut error = ptr::null_mut();
    // SAFETY: `object` is a live accessible proxy.
    let process_id = unsafe { atspi_accessible_get_process_id(object, &raw mut error) };
    if !error.is_null() {
        // SAFETY: GLib returned this error to the caller.
        unsafe { g_error_free(error) };
        return 0;
    }
    process_id
}

fn states(object: *mut AtspiAccessible) -> Vec<&'static str> {
    // SAFETY: `object` is live; the returned state set is an owned GObject.
    let set = unsafe { atspi_accessible_get_state_set(object) };
    if set.is_null() {
        return Vec::new();
    }
    let mut result = Vec::new();
    for (value, name) in [
        (STATE_ACTIVE, "active"),
        (STATE_ENABLED, "enabled"),
        (STATE_FOCUSABLE, "focusable"),
        (STATE_FOCUSED, "focused"),
        (STATE_SELECTED, "selected"),
        (STATE_SHOWING, "showing"),
        (STATE_VISIBLE, "visible"),
    ] {
        // SAFETY: `set` remains live for this loop and `value` is a valid enum.
        if unsafe { atspi_state_set_contains(set, value) } != 0 {
            result.push(name);
        }
    }
    // SAFETY: The state set is an owned GObject reference.
    unsafe { g_object_unref(set.cast()) };
    result
}

fn actions(object: *mut AtspiAccessible) -> Vec<String> {
    // SAFETY: `object` is live; AT-SPI returns a full interface reference.
    let action = unsafe { atspi_accessible_get_action_iface(object) };
    if action.is_null() {
        return Vec::new();
    }
    let _owned_action = OwnedObject(action.cast());
    let mut error = ptr::null_mut();
    // SAFETY: `action` remains owned by the accessible for this call.
    let count = unsafe { atspi_action_get_n_actions(action, &raw mut error) };
    if !error.is_null() {
        // SAFETY: GLib returned this error to the caller.
        unsafe { g_error_free(error) };
        return Vec::new();
    }
    (0..count.max(0))
        .map(|index| {
            let mut error = ptr::null_mut();
            // SAFETY: The index is bounded by the just-read action count.
            let name = unsafe { atspi_action_get_action_name(action, index, &raw mut error) };
            if error.is_null() {
                take_string(name)
            } else {
                // SAFETY: GLib returned this error to the caller.
                unsafe { g_error_free(error) };
                String::new()
            }
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn inspect(object: *mut AtspiAccessible, depth: usize) -> Value {
    let name = call_string(atspi_accessible_get_name, object);
    let description = call_string(atspi_accessible_get_description, object);
    let role = call_string(atspi_accessible_get_role_name, object);
    let mut children = Vec::new();
    if depth < 32 {
        for index in 0..child_count(object) {
            let mut error = ptr::null_mut();
            // SAFETY: The index is bounded by the current child count.
            let child =
                unsafe { atspi_accessible_get_child_at_index(object, index, &raw mut error) };
            if !error.is_null() {
                // SAFETY: GLib returned this error to the caller.
                unsafe { g_error_free(error) };
                continue;
            }
            if !child.is_null() {
                let child = OwnedAccessible(child);
                children.push(inspect(child.0, depth + 1));
            }
        }
    }
    json!({
        "name": name,
        "description": description,
        "role": role,
        "process_id": process_id(object),
        "states": states(object),
        "actions": actions(object),
        "children": children,
    })
}

fn tree_contains_name(value: &Value, expected: &str) -> bool {
    match value {
        Value::Object(fields) => {
            fields.get("name").and_then(Value::as_str) == Some(expected)
                || fields
                    .values()
                    .any(|child| tree_contains_name(child, expected))
        }
        Value::Array(values) => values
            .iter()
            .any(|child| tree_contains_name(child, expected)),
        _ => false,
    }
}

fn root_matches(object: *mut AtspiAccessible, expected_name: &str, expected_pid: u32) -> bool {
    process_id(object) == expected_pid
        && call_string(atspi_accessible_get_name, object) == expected_name
}

fn snapshot(expected_name: &str, expected_pid: u32) -> Value {
    // SAFETY: This reads the initialized registry's desktop count.
    let count = unsafe { atspi_get_desktop_count() };
    let mut applications = Vec::new();
    for desktop_index in 0..count.max(0) {
        // SAFETY: The index is bounded by the current desktop count. AT-SPI
        // returns a full desktop proxy reference.
        let desktop = unsafe { atspi_get_desktop(desktop_index) };
        if desktop.is_null() {
            continue;
        }
        let desktop = OwnedAccessible(desktop);
        for index in 0..child_count(desktop.0) {
            let mut error = ptr::null_mut();
            // SAFETY: The child index is bounded by the desktop child count.
            let child =
                unsafe { atspi_accessible_get_child_at_index(desktop.0, index, &raw mut error) };
            if !error.is_null() {
                // SAFETY: GLib returned this error to the caller.
                unsafe { g_error_free(error) };
                continue;
            }
            if !child.is_null() {
                let child = OwnedAccessible(child);
                if root_matches(child.0, expected_name, expected_pid) {
                    applications.push(inspect(child.0, 0));
                }
            }
        }
    }
    json!({"schema_version": 1, "applications": applications})
}

fn click_named(object: *mut AtspiAccessible, expected: &str) -> bool {
    if call_string(atspi_accessible_get_name, object) == expected {
        // SAFETY: The accessible is live and returns a full action reference.
        let action = unsafe { atspi_accessible_get_action_iface(object) };
        if !action.is_null() {
            let _owned_action = OwnedObject(action.cast());
            let mut error = ptr::null_mut();
            // SAFETY: The action interface is live for this call.
            let count = unsafe { atspi_action_get_n_actions(action, &raw mut error) };
            if error.is_null() {
                for index in 0..count.max(0) {
                    let mut error = ptr::null_mut();
                    // SAFETY: The index is bounded by the current action count.
                    let name =
                        unsafe { atspi_action_get_action_name(action, index, &raw mut error) };
                    if !error.is_null() {
                        // SAFETY: GLib returned this error to the caller.
                        unsafe { g_error_free(error) };
                        continue;
                    }
                    let name = take_string(name);
                    if name == "click" || name == "default.activate" {
                        let mut error = ptr::null_mut();
                        // SAFETY: Invoke the named user-facing action at its
                        // still-bounded index through the external AT-SPI API.
                        let activated =
                            unsafe { atspi_action_do_action(action, index, &raw mut error) } != 0;
                        if !error.is_null() {
                            // SAFETY: GLib returned this error to the caller.
                            unsafe { g_error_free(error) };
                            return false;
                        }
                        return activated;
                    }
                }
            } else {
                // SAFETY: GLib returned this error to the caller.
                unsafe { g_error_free(error) };
            }
        }
    }

    for index in 0..child_count(object) {
        let mut error = ptr::null_mut();
        // SAFETY: The index is bounded by the current child count.
        let child = unsafe { atspi_accessible_get_child_at_index(object, index, &raw mut error) };
        if !error.is_null() {
            // SAFETY: GLib returned this error to the caller.
            unsafe { g_error_free(error) };
            continue;
        }
        if !child.is_null() {
            let child = OwnedAccessible(child);
            if click_named(child.0, expected) {
                return true;
            }
        }
    }
    false
}

fn activate(expected_name: &str, expected_pid: u32, target: &str) -> bool {
    // SAFETY: This reads the initialized registry's desktop count.
    let count = unsafe { atspi_get_desktop_count() };
    for desktop_index in 0..count.max(0) {
        // SAFETY: The index is bounded by the current desktop count.
        let desktop = unsafe { atspi_get_desktop(desktop_index) };
        if desktop.is_null() {
            continue;
        }
        let desktop = OwnedAccessible(desktop);
        for index in 0..child_count(desktop.0) {
            let mut error = ptr::null_mut();
            // SAFETY: The index is bounded by the current child count.
            let child =
                unsafe { atspi_accessible_get_child_at_index(desktop.0, index, &raw mut error) };
            if !error.is_null() {
                // SAFETY: GLib returned this error to the caller.
                unsafe { g_error_free(error) };
                continue;
            }
            if !child.is_null() {
                let child = OwnedAccessible(child);
                if root_matches(child.0, expected_name, expected_pid)
                    && click_named(child.0, target)
                {
                    return true;
                }
            }
        }
    }
    false
}

fn main() {
    let mut arguments = env::args().skip(1);
    let mode = arguments.next().unwrap_or_default();
    let expected = arguments.next().unwrap_or_default();
    let expected_pid = arguments
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let target = (mode == "activate").then(|| arguments.next().unwrap_or_default());
    let timeout_argument = arguments.next();
    let parsed_timeout = timeout_argument
        .as_deref()
        .map(str::parse::<u64>)
        .transpose();
    let invalid_timeout = parsed_timeout.is_err();
    let timeout_ms = parsed_timeout.ok().flatten().unwrap_or(5_000);
    if !matches!(mode.as_str(), "snapshot" | "activate")
        || expected.is_empty()
        || expected_pid == 0
        || target.as_ref().is_some_and(String::is_empty)
        || invalid_timeout
        || arguments.next().is_some()
    {
        eprintln!(
            "usage: zentty-atspi-inspector snapshot APPLICATION PID [TIMEOUT_MS]\n       zentty-atspi-inspector activate APPLICATION PID TARGET [TIMEOUT_MS]"
        );
        std::process::exit(64);
    }

    // SAFETY: AT-SPI initialization is process-global and called once before
    // any registry access in this short-lived inspector.
    if unsafe { atspi_init() } != 0 {
        eprintln!("zentty-atspi-inspector: AT-SPI initialization failed");
        std::process::exit(1);
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let receipt = snapshot(&expected, expected_pid);
        let succeeded = if let Some(target) = target.as_deref() {
            tree_contains_name(&receipt, target) && activate(&expected, expected_pid, target)
        } else {
            !receipt["applications"].as_array().is_none_or(Vec::is_empty)
        };
        if succeeded {
            println!("{receipt}");
            // SAFETY: Balance the single successful initialization above.
            unsafe { atspi_exit() };
            return;
        }
        if Instant::now() >= deadline {
            println!("{receipt}");
            eprintln!(
                "zentty-atspi-inspector: expected application={expected:?} pid={expected_pid} target={target:?} was absent or inactive"
            );
            // SAFETY: Balance the single successful initialization above.
            unsafe { atspi_exit() };
            std::process::exit(1);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::tree_contains_name;
    use serde_json::json;

    #[test]
    fn name_search_crosses_receipt_and_accessible_children() {
        let receipt = json!({
            "applications": [{
                "name": "zentty",
                "children": [{"name": "Add Pane Right", "children": []}]
            }]
        });
        assert!(tree_contains_name(&receipt, "zentty"));
        assert!(tree_contains_name(&receipt, "Add Pane Right"));
        assert!(!tree_contains_name(&receipt, "Another application"));
    }

    #[test]
    fn unrelated_scalar_values_cannot_impersonate_accessible_names() {
        let receipt = json!({
            "application_hint": "zentty",
            "applications": [{"name": "different", "children": []}]
        });
        assert!(!tree_contains_name(&receipt, "zentty"));
    }
}
