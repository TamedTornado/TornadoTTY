//! Raw declarations for the exact Ghostty GTK embedding C ABI.

use core::ffi::{c_char, c_void};

#[repr(C)]
pub struct GhosttyGtkEmbedRuntime {
    _private: [u8; 0],
}

pub type GtkWidget = c_void;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum GhosttyGtkEmbedAsyncBackend {
    Default = 0,
    Epoll = 1,
    IoUring = 2,
}

unsafe extern "C" {
    pub fn ghostty_gtk_embed_runtime_new() -> *mut GhosttyGtkEmbedRuntime;
    pub fn ghostty_gtk_embed_runtime_new_with_async_backend(
        backend: GhosttyGtkEmbedAsyncBackend,
    ) -> *mut GhosttyGtkEmbedRuntime;
    pub fn ghostty_gtk_embed_runtime_free(runtime: *mut GhosttyGtkEmbedRuntime);
    pub fn ghostty_gtk_embed_runtime_tick(runtime: *mut GhosttyGtkEmbedRuntime) -> bool;
    pub fn ghostty_gtk_embed_surface_new(
        runtime: *mut GhosttyGtkEmbedRuntime,
        command: *const c_char,
        title: *const c_char,
    ) -> *mut GtkWidget;
    pub fn ghostty_gtk_embed_surface_grab_focus(surface: *mut GtkWidget);
    pub fn ghostty_gtk_embed_surface_close(surface: *mut GtkWidget) -> bool;
    pub fn ghostty_gtk_embed_surface_send_text(
        surface: *mut GtkWidget,
        text: *const c_char,
    ) -> bool;
    pub fn ghostty_gtk_embed_surface_binding_action(
        surface: *mut GtkWidget,
        action: *const c_char,
        action_len: usize,
    ) -> bool;
    pub fn ghostty_gtk_embed_surface_request_paste(surface: *mut GtkWidget) -> bool;
}

#[cfg(test)]
mod tests {
    use super::GhosttyGtkEmbedAsyncBackend;

    #[test]
    fn async_backend_is_fixed_width_and_matches_c_int_values() {
        assert_eq!(size_of::<GhosttyGtkEmbedAsyncBackend>(), size_of::<i32>());
        assert_eq!(GhosttyGtkEmbedAsyncBackend::Default as i32, 0);
        assert_eq!(GhosttyGtkEmbedAsyncBackend::Epoll as i32, 1);
        assert_eq!(GhosttyGtkEmbedAsyncBackend::IoUring as i32, 2);
    }
}
