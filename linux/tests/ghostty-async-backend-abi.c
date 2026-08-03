#include <ghostty/gtk.h>

#include <stdio.h>

#ifdef __cplusplus
static_assert(GHOSTTY_GTK_EMBED_ASYNC_DEFAULT == 0, "default async discriminant changed");
static_assert(GHOSTTY_GTK_EMBED_ASYNC_EPOLL == 1, "epoll async discriminant changed");
static_assert(GHOSTTY_GTK_EMBED_ASYNC_IO_URING == 2, "io_uring async discriminant changed");
#define ZENTTY_ABI_LANGUAGE "c++17"
#else
_Static_assert(GHOSTTY_GTK_EMBED_ASYNC_DEFAULT == 0, "default async discriminant changed");
_Static_assert(GHOSTTY_GTK_EMBED_ASYNC_EPOLL == 1, "epoll async discriminant changed");
_Static_assert(GHOSTTY_GTK_EMBED_ASYNC_IO_URING == 2, "io_uring async discriminant changed");
#define ZENTTY_ABI_LANGUAGE "c17"
#endif

int main(void) {
    printf(
        "async-backend-abi: language=%s enum_size=%zu c_int_size=%zu values=%d,%d,%d\n",
        ZENTTY_ABI_LANGUAGE,
        sizeof(ghostty_gtk_embed_async_backend_t),
        sizeof(int),
        (int) GHOSTTY_GTK_EMBED_ASYNC_DEFAULT,
        (int) GHOSTTY_GTK_EMBED_ASYNC_EPOLL,
        (int) GHOSTTY_GTK_EMBED_ASYNC_IO_URING
    );
    return 0;
}
