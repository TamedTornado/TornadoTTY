#include <ghostty/gtk.h>

#include <cstdint>
#include <type_traits>

static_assert(std::is_enum_v<ghostty_gtk_embed_async_backend_t>);
static_assert(GHOSTTY_GTK_EMBED_ASYNC_DEFAULT == 0);
static_assert(GHOSTTY_GTK_EMBED_ASYNC_EPOLL == 1);
static_assert(GHOSTTY_GTK_EMBED_ASYNC_IO_URING == 2);
static_assert(std::is_same_v<ghostty_gtk_embed_text_extent_t, std::uint32_t>);
static_assert(GHOSTTY_GTK_EMBED_TEXT_VIEWPORT == 0);
static_assert(GHOSTTY_GTK_EMBED_TEXT_SCREEN == 1);

using RuntimeConstructor = ghostty_gtk_embed_runtime_t *(*)();
using SurfaceConstructor = GtkWidget *(*) (
    ghostty_gtk_embed_runtime_t *,
    const char *,
    const char *
);
using SurfaceOptionsConstructor = GtkWidget *(*) (
    ghostty_gtk_embed_runtime_t *,
    const ghostty_gtk_embed_surface_options_t *
);
using SurfaceTextReader = bool (*) (
    GtkWidget *,
    ghostty_gtk_embed_text_extent_t,
    ghostty_gtk_embed_text_callback_t,
    void *
);
using SurfaceSelectionReader = bool (*) (
    GtkWidget *,
    ghostty_gtk_embed_text_callback_t,
    void *
);

static_assert(std::is_same_v<
    decltype(&ghostty_gtk_embed_runtime_new),
    RuntimeConstructor
>);
static_assert(std::is_standard_layout_v<ghostty_gtk_embed_surface_options_t>);
static_assert(
    offsetof(ghostty_gtk_embed_surface_options_t, environment) >
    offsetof(ghostty_gtk_embed_surface_options_t, working_directory)
);
static_assert(
    offsetof(ghostty_gtk_embed_surface_options_t, environment_count) >
    offsetof(ghostty_gtk_embed_surface_options_t, environment)
);
static_assert(std::is_same_v<
    decltype(&ghostty_gtk_embed_surface_new_with_options),
    SurfaceOptionsConstructor
>);
static_assert(std::is_same_v<
    decltype(&ghostty_gtk_embed_surface_new),
    SurfaceConstructor
>);
static_assert(std::is_same_v<
    decltype(&ghostty_gtk_embed_surface_read_text),
    SurfaceTextReader
>);
static_assert(std::is_same_v<
    decltype(&ghostty_gtk_embed_surface_read_selection),
    SurfaceSelectionReader
>);
