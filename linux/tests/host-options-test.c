#include "host_options.h"

#include <stdio.h>

static int test_terminal_counts(void) {
    unsigned int count = 99;
    if (!zentty_parse_terminal_count(NULL, &count) || count != 1) {
        return 1;
    }
    for (unsigned int expected = 1; expected <= 4; expected++) {
        const char value[] = {(char) ('0' + expected), '\0'};
        if (!zentty_parse_terminal_count(value, &count) || count != expected) {
            return 1;
        }
    }
    const char *invalid[] = {
        "", "0", "5", "-1", "+1", " 1", "1 ", "01", "10",
        "999999999999999999999999999999999999999",
    };
    for (unsigned int index = 0;
        index < sizeof(invalid) / sizeof(invalid[0]);
        index++) {
        if (zentty_parse_terminal_count(invalid[index], &count)) {
            return 1;
        }
    }
    return zentty_parse_terminal_count("1", NULL) ? 1 : 0;
}

static int test_async_backends(void) {
    ghostty_gtk_embed_async_backend_t backend;
    if (!zentty_parse_async_backend(NULL, &backend) ||
        backend != GHOSTTY_GTK_EMBED_ASYNC_DEFAULT ||
        !zentty_parse_async_backend("auto", &backend) ||
        backend != GHOSTTY_GTK_EMBED_ASYNC_DEFAULT ||
        !zentty_parse_async_backend("epoll", &backend) ||
        backend != GHOSTTY_GTK_EMBED_ASYNC_EPOLL ||
        !zentty_parse_async_backend("io_uring", &backend) ||
        backend != GHOSTTY_GTK_EMBED_ASYNC_IO_URING) {
        return 1;
    }
    const char *invalid[] = {"", "default", "EPOLL", "uring", " epoll"};
    for (unsigned int index = 0;
        index < sizeof(invalid) / sizeof(invalid[0]);
        index++) {
        if (zentty_parse_async_backend(invalid[index], &backend)) {
            return 1;
        }
    }
    return zentty_parse_async_backend("auto", NULL) ? 1 : 0;
}

int main(void) {
    if (test_terminal_counts() != 0 || test_async_backends() != 0) {
        fputs("host-options-test: FAIL\n", stderr);
        return 1;
    }
    puts("host-options-test: PASS");
    return 0;
}
