//! The X11 display class is separate from `GLib`'s program/instance name.
use glib::translate::ToGlibPtr;
use gtk::{gdk, glib, prelude::*};

unsafe extern "C" {
    fn gdk_x11_display_set_program_class(
        display: *mut gdk::ffi::GdkDisplay,
        program_class: *const std::ffi::c_char,
    );
}

pub(super) fn set_program_class(application_id: &str) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let Some(x11_type) = glib::Type::from_name("GdkX11Display") else {
        return;
    };
    if !display.type_().is_a(x11_type) {
        return;
    }
    // SAFETY: The runtime owner calls this on the GTK thread before creating
    // host windows. The live display is checked to be an X11 display, and
    // ToGlibPtr keeps the NUL-terminated class alive for this copying setter.
    unsafe {
        gdk_x11_display_set_program_class(
            display.to_glib_none().0,
            application_id.to_glib_none().0,
        );
    }
}
