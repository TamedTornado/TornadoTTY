#ifndef ZENTTY_LINUX_HOST_OPTIONS_H
#define ZENTTY_LINUX_HOST_OPTIONS_H

#include <ghostty/gtk.h>

#include <stdbool.h>

bool zentty_parse_terminal_count(const char *value, unsigned int *count);
bool zentty_parse_async_backend(
    const char *value,
    ghostty_gtk_embed_async_backend_t *backend
);

#endif
