#include "host_options.h"

#include <string.h>

#define ZENTTY_MAX_TERMINALS 4

bool zentty_parse_terminal_count(const char *value, unsigned int *count) {
    if (count == NULL) {
        return false;
    }
    if (value == NULL) {
        *count = 1;
        return true;
    }
    if (value[0] < '1' || value[0] > '4' || value[1] != '\0') {
        return false;
    }

    *count = (unsigned int) (value[0] - '0');
    return *count <= ZENTTY_MAX_TERMINALS;
}

bool zentty_parse_async_backend(
    const char *value,
    ghostty_gtk_embed_async_backend_t *backend
) {
    if (backend == NULL) {
        return false;
    }
    if (value == NULL || strcmp(value, "auto") == 0) {
        *backend = GHOSTTY_GTK_EMBED_ASYNC_DEFAULT;
        return true;
    }
    if (strcmp(value, "epoll") == 0) {
        *backend = GHOSTTY_GTK_EMBED_ASYNC_EPOLL;
        return true;
    }
    if (strcmp(value, "io_uring") == 0) {
        *backend = GHOSTTY_GTK_EMBED_ASYNC_IO_URING;
        return true;
    }
    return false;
}
