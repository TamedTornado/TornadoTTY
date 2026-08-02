#include <ghostty/gtk.h>

#include <type_traits>

static_assert(std::is_enum_v<ghostty_gtk_embed_async_backend_t>);
static_assert(GHOSTTY_GTK_EMBED_ASYNC_DEFAULT == 0);
static_assert(GHOSTTY_GTK_EMBED_ASYNC_EPOLL == 1);
static_assert(GHOSTTY_GTK_EMBED_ASYNC_IO_URING == 2);

using RuntimeConstructor = ghostty_gtk_embed_runtime_t *(*)();
using SurfaceConstructor = GtkWidget *(*) (
    ghostty_gtk_embed_runtime_t *,
    const char *,
    const char *
);

static_assert(std::is_same_v<
    decltype(&ghostty_gtk_embed_runtime_new),
    RuntimeConstructor
>);
static_assert(std::is_same_v<
    decltype(&ghostty_gtk_embed_surface_new),
    SurfaceConstructor
>);
