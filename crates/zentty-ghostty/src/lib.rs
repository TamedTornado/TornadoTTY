#![deny(unsafe_op_in_unsafe_fn)]

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::ffi::{CString, NulError};
use std::fmt;
use std::ptr::NonNull;
use std::rc::Rc;
use std::string::FromUtf8Error;
use zentty_ghostty_sys as sys;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AsyncBackend {
    #[default]
    Default,
    Epoll,
    IoUring,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextExtent {
    Viewport,
    Screen,
}

impl From<TextExtent> for sys::GhosttyGtkEmbedTextExtent {
    fn from(extent: TextExtent) -> Self {
        match extent {
            TextExtent::Viewport => Self::Viewport,
            TextExtent::Screen => Self::Screen,
        }
    }
}

impl From<AsyncBackend> for sys::GhosttyGtkEmbedAsyncBackend {
    fn from(backend: AsyncBackend) -> Self {
        match backend {
            AsyncBackend::Default => Self::Default,
            AsyncBackend::Epoll => Self::Epoll,
            AsyncBackend::IoUring => Self::IoUring,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    ConstructorFailed {
        backend: AsyncBackend,
    },
    InteriorNul {
        field: &'static str,
        source: NulError,
    },
    InvalidEnvironmentName(String),
    TooManyEnvironmentEntries(usize),
    SurfaceConstructorFailed,
    UnexpectedSurfaceTransfer,
    SurfaceCloseFailed,
    InputFailed,
    BindingActionFailed,
    TextReadFailed,
    InvalidText(FromUtf8Error),
    TickFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConstructorFailed { backend } => {
                write!(
                    formatter,
                    "Ghostty runtime construction failed for {backend:?}"
                )
            }
            Self::InteriorNul { field, .. } => {
                write!(
                    formatter,
                    "Ghostty surface {field} contains an interior NUL"
                )
            }
            Self::InvalidEnvironmentName(name) => {
                write!(
                    formatter,
                    "Ghostty surface environment name is invalid: {name:?}"
                )
            }
            Self::TooManyEnvironmentEntries(count) => write!(
                formatter,
                "Ghostty surface environment has {count} entries; maximum is 128"
            ),
            Self::SurfaceConstructorFailed => {
                formatter.write_str("Ghostty surface construction failed")
            }
            Self::UnexpectedSurfaceTransfer => {
                formatter.write_str("Ghostty surface constructor did not return a floating widget")
            }
            Self::SurfaceCloseFailed => formatter.write_str("Ghostty surface close failed"),
            Self::InputFailed => formatter.write_str("Ghostty terminal input failed"),
            Self::BindingActionFailed => {
                formatter.write_str("Ghostty terminal binding action failed")
            }
            Self::TextReadFailed => formatter.write_str("Ghostty terminal text read failed"),
            Self::InvalidText(_) => {
                formatter.write_str("Ghostty terminal text was not valid UTF-8")
            }
            Self::TickFailed => formatter.write_str("Ghostty runtime tick failed"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InteriorNul { source, .. } => Some(source),
            Self::InvalidText(source) => Some(source),
            _ => None,
        }
    }
}

struct RuntimeInner {
    raw: NonNull<sys::GhosttyGtkEmbedRuntime>,
    _main_thread: Rc<()>,
}

impl Drop for RuntimeInner {
    fn drop(&mut self) {
        // SAFETY: `raw` came from the one successful Ghostty constructor. The
        // last `Rc<RuntimeInner>` cannot drop until every `GhosttySurface`
        // releases its runtime lease.
        unsafe { sys::ghostty_gtk_embed_runtime_free(self.raw.as_ptr()) };
    }
}

/// A lease on Ghostty's process-global runtime.
///
/// Runtime operations are restricted to the owning `GLib` main thread.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<zentty_ghostty::GhosttyRuntime>();
/// ```
///
/// ```compile_fail
/// fn require_sync<T: Sync>() {}
/// require_sync::<zentty_ghostty::GhosttyRuntime>();
/// ```
#[derive(Clone)]
pub struct GhosttyRuntime {
    inner: Rc<RuntimeInner>,
}

impl GhosttyRuntime {
    /// Creates the process-global Ghostty runtime before GTK initialization.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConstructorFailed`] when Ghostty rejects the backend,
    /// initialization order, or second-runtime attempt.
    pub fn new(backend: AsyncBackend) -> Result<Self, Error> {
        // SAFETY: This is the sole raw constructor call exposed by the safe
        // adapter. The caller must invoke it before GTK initialization; native
        // Ghostty rejects unavailable backends and repeated construction.
        let raw = unsafe { sys::ghostty_gtk_embed_runtime_new_with_async_backend(backend.into()) };
        let raw = NonNull::new(raw).ok_or(Error::ConstructorFailed { backend })?;
        Ok(Self {
            inner: Rc::new(RuntimeInner {
                raw,
                _main_thread: Rc::new(()),
            }),
        })
    }

    /// Drives the native application mailbox from the `GLib` main loop.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TickFailed`] when the native runtime rejects the tick.
    pub fn tick(&self) -> Result<(), Error> {
        // SAFETY: The runtime pointer stays valid for the lifetime of `inner`;
        // `Rc` makes the adapter main-thread-only.
        let succeeded = unsafe { sys::ghostty_gtk_embed_runtime_tick(self.inner.raw.as_ptr()) };
        succeeded.then_some(()).ok_or(Error::TickFailed)
    }

    /// Creates one GTK-owned Ghostty surface after GTK initialization.
    ///
    /// # Errors
    ///
    /// Returns a contextual construction or string-conversion error.
    pub fn create_surface(&self, config: &SurfaceConfig) -> Result<GhosttySurface, Error> {
        let command = config
            .command
            .as_deref()
            .map(CString::new)
            .transpose()
            .map_err(|source| Error::InteriorNul {
                field: "command",
                source,
            })?;
        let title = CString::new(config.title.as_str()).map_err(|source| Error::InteriorNul {
            field: "title",
            source,
        })?;
        let working_directory = config
            .working_directory
            .as_deref()
            .map(CString::new)
            .transpose()
            .map_err(|source| Error::InteriorNul {
                field: "working_directory",
                source,
            })?;
        let environment = encode_environment(&config.environment)?;
        let environment_pointers = environment
            .iter()
            .map(|value| value.as_ptr())
            .collect::<Vec<_>>();
        let options = sys::GhosttyGtkEmbedSurfaceOptions {
            struct_size: std::mem::size_of::<sys::GhosttyGtkEmbedSurfaceOptions>(),
            command: command
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            title: title.as_ptr(),
            working_directory: working_directory
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            environment: if environment_pointers.is_empty() {
                std::ptr::null()
            } else {
                environment_pointers.as_ptr()
            },
            environment_count: environment_pointers.len(),
        };

        // SAFETY: Runtime validity is held by `inner`; `CString` pointers remain
        // valid for this call and Ghostty documents that both strings are
        // copied. A successful return transfers one full GObject reference.
        let raw = unsafe {
            sys::ghostty_gtk_embed_surface_new_with_options(
                self.inner.raw.as_ptr(),
                &raw const options,
            )
        };
        let raw = NonNull::new(raw).ok_or(Error::SurfaceConstructorFailed)?;
        // The current Ghostty implementation constructs a `GtkWidget`, which
        // is floating. The public header does not yet promise transfer mode,
        // so fail before wrapping if the pinned implementation changes.
        // SAFETY: `raw` is a live GObject returned by the constructor.
        let is_floating =
            unsafe { glib::gobject_ffi::g_object_is_floating(raw.as_ptr().cast()).is_positive() };
        if !is_floating {
            // SAFETY: The constructor returned a new non-floating object. It
            // cannot cross the safe boundary without a documented transfer
            // contract, so release that constructor reference and fail.
            unsafe { glib::gobject_ffi::g_object_unref(raw.as_ptr().cast()) };
            return Err(Error::UnexpectedSurfaceTransfer);
        }
        // SAFETY: The runtime check above proves the returned widget is
        // floating. `from_glib_none` sinks that floating reference and gives
        // the Rust wrapper exactly one owned reference before any container
        // sees the widget.
        let widget: gtk::Widget =
            unsafe { glib::translate::from_glib_none(raw.as_ptr().cast::<gtk::ffi::GtkWidget>()) };
        // GTK/GSK may retain a widget beyond the host wrapper's final strong
        // reference (for example, until an unmapped GL area's last frame is
        // retired). Tie a runtime lease to GObject finalization itself so
        // native global teardown cannot race those external references.
        // SAFETY: This adapter exclusively owns this private key, never reads
        // or replaces its value, and stores the single declared Rust type.
        unsafe {
            widget.set_qdata(
                glib::Quark::from_str("zentty-ghostty-runtime-lease"),
                Rc::clone(&self.inner),
            );
        }
        Ok(GhosttySurface {
            widget,
            handlers: RefCell::new(Vec::new()),
            _runtime: Rc::clone(&self.inner),
        })
    }
}

fn encode_environment(environment: &[(String, String)]) -> Result<Vec<CString>, Error> {
    if environment.len() > 128 {
        return Err(Error::TooManyEnvironmentEntries(environment.len()));
    }
    environment
        .iter()
        .map(|(name, value)| {
            if name.is_empty() || name.contains('=') {
                return Err(Error::InvalidEnvironmentName(name.clone()));
            }
            CString::new(format!("{name}={value}")).map_err(|source| Error::InteriorNul {
                field: "environment",
                source,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceConfig {
    pub command: Option<String>,
    pub title: String,
    pub working_directory: Option<String>,
    pub environment: Vec<(String, String)>,
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self {
            command: None,
            title: "Zentty".to_owned(),
            working_directory: None,
            environment: Vec::new(),
        }
    }
}

/// A main-thread terminal surface holding a runtime lease.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<zentty_ghostty::GhosttySurface>();
/// ```
pub struct GhosttySurface {
    widget: gtk::Widget,
    handlers: RefCell<Vec<glib::SignalHandlerId>>,
    _runtime: Rc<RuntimeInner>,
}

impl GhosttySurface {
    #[must_use]
    pub fn widget(&self) -> &gtk::Widget {
        &self.widget
    }

    pub fn grab_focus(&self) {
        // SAFETY: `widget` owns a live Ghostty surface reference and the `Rc`
        // runtime lease prevents native teardown during the call.
        unsafe { sys::ghostty_gtk_embed_surface_grab_focus(self.widget.as_ptr().cast()) };
    }

    /// Disposes a detached native widget and consumes the host wrapper.
    ///
    /// The adapter disconnects its callbacks first and consuming `self`
    /// prevents host use of the non-functional object. The `GObject` qdata
    /// runtime lease remains until finalization if GTK/GSK retains the widget.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SurfaceCloseFailed`] if native Ghostty rejects the
    /// explicit embedding lifecycle transition.
    pub fn dispose(mut self) -> Result<(), Error> {
        for handler in self.handlers.get_mut().drain(..) {
            self.widget.disconnect(handler);
        }
        // SAFETY: The live widget came from the embedding constructor and has
        // been detached by the composition root before this consuming call.
        if !unsafe { sys::ghostty_gtk_embed_surface_close(self.widget.as_ptr().cast()) } {
            return Err(Error::SurfaceCloseFailed);
        }
        // SAFETY: The composition root has detached the widget, and consuming
        // the only safe host handle prevents use after GObject disposal.
        unsafe { self.widget.run_dispose() };
        Ok(())
    }

    /// Sends UTF-8 text through Ghostty's terminal input path.
    ///
    /// # Errors
    ///
    /// Returns a string-conversion or native input error.
    pub fn send_text(&self, text: &str) -> Result<(), Error> {
        let text = CString::new(text).map_err(|source| Error::InteriorNul {
            field: "input text",
            source,
        })?;
        // SAFETY: The widget and text pointers are live for the call; Ghostty
        // consumes the text synchronously through the surface input path.
        let succeeded = unsafe {
            sys::ghostty_gtk_embed_surface_send_text(self.widget.as_ptr().cast(), text.as_ptr())
        };
        succeeded.then_some(()).ok_or(Error::InputFailed)
    }

    /// Invokes a Ghostty binding action through the terminal core's native
    /// parser and dispatcher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BindingActionFailed`] when the action is malformed,
    /// unsupported, or rejected by the live terminal surface.
    pub fn perform_binding_action(&self, action: &str) -> Result<(), Error> {
        // SAFETY: `widget` owns a live Ghostty surface reference. The action
        // bytes remain valid for the synchronous call, and the explicit byte
        // length means no C-string conversion or trailing NUL is required.
        let succeeded = unsafe {
            sys::ghostty_gtk_embed_surface_binding_action(
                self.widget.as_ptr().cast(),
                action.as_ptr().cast(),
                action.len(),
            )
        };
        succeeded.then_some(()).ok_or(Error::BindingActionFailed)
    }

    /// Copies plain terminal text from the selected Ghostty extent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TextReadFailed`] when the native surface cannot read
    /// the requested extent, or [`Error::InvalidText`] if the terminal breaks
    /// its UTF-8 text contract.
    pub fn read_text(&self, extent: TextExtent) -> Result<String, Error> {
        unsafe extern "C" fn copy_text(
            text: *const core::ffi::c_char,
            text_len: usize,
            userdata: *mut core::ffi::c_void,
        ) {
            // SAFETY: The native API invokes this synchronously with the
            // `Vec<u8>` passed below and a readable byte range valid for the
            // duration of this callback.
            let output = unsafe { &mut *userdata.cast::<Vec<u8>>() };
            // SAFETY: Ghostty's callback contract provides `text_len`
            // readable bytes, including for text containing interior NULs.
            if text_len > 0 {
                let bytes = unsafe { std::slice::from_raw_parts(text.cast(), text_len) };
                output.extend_from_slice(bytes);
            }
        }

        let mut bytes = Vec::new();
        // SAFETY: `widget` owns a live Ghostty surface. `bytes` remains live
        // for the synchronous call, and `copy_text` does not retain either
        // borrowed native text or its userdata pointer.
        let succeeded = unsafe {
            sys::ghostty_gtk_embed_surface_read_text(
                self.widget.as_ptr().cast(),
                extent.into(),
                Some(copy_text),
                (&raw mut bytes).cast(),
            )
        };
        if !succeeded {
            return Err(Error::TextReadFailed);
        }
        String::from_utf8(bytes).map_err(Error::InvalidText)
    }

    pub fn on_initialized(&self, callback: impl Fn() + 'static) {
        let handler = self.widget.connect_local("init", false, move |_| {
            callback();
            None
        });
        self.handlers.borrow_mut().push(handler);
    }

    pub fn on_title_changed(&self, callback: impl Fn(String) + 'static) {
        let handler = self
            .widget
            .connect_notify_local(Some("title"), move |widget, _| {
                callback(widget.property::<String>("title"));
            });
        self.handlers.borrow_mut().push(handler);
    }

    pub fn on_child_exited(&self, callback: impl Fn() + 'static) {
        let handler = self
            .widget
            .connect_notify_local(Some("child-exited"), move |widget, _| {
                if widget.property::<bool>("child-exited") {
                    callback();
                }
            });
        self.handlers.borrow_mut().push(handler);
    }
}

impl Drop for GhosttySurface {
    fn drop(&mut self) {
        for handler in self.handlers.get_mut().drain(..) {
            self.widget.disconnect(handler);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, encode_environment};

    #[test]
    fn surface_environment_is_encoded_as_borrowed_key_value_entries() {
        let encoded =
            encode_environment(&[("ZENTTY_PANE_ID".to_owned(), "pane-1".to_owned())]).unwrap();
        assert_eq!(encoded[0].to_str().unwrap(), "ZENTTY_PANE_ID=pane-1");
    }

    #[test]
    fn surface_environment_rejects_invalid_names_nuls_and_excessive_counts() {
        assert!(matches!(
            encode_environment(&[("BAD=NAME".to_owned(), "value".to_owned())]),
            Err(Error::InvalidEnvironmentName(_))
        ));
        assert!(matches!(
            encode_environment(&[("GOOD".to_owned(), "bad\0value".to_owned())]),
            Err(Error::InteriorNul {
                field: "environment",
                ..
            })
        ));
        let too_many = (0..129)
            .map(|index| (format!("KEY_{index}"), "value".to_owned()))
            .collect::<Vec<_>>();
        assert!(matches!(
            encode_environment(&too_many),
            Err(Error::TooManyEnvironmentEntries(129))
        ));
    }
}
