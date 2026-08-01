#include <ghostty/gtk.h>

#include <stdio.h>

int main(void) {
    if (ghostty_gtk_embed_runtime_new_with_async_backend(
            (ghostty_gtk_embed_async_backend_t) 999) != NULL) {
        fputs("api-contract: invalid async backend accepted\n", stderr);
        return 1;
    }
    ghostty_gtk_embed_runtime_free(NULL);
    if (ghostty_gtk_embed_runtime_tick(NULL)) {
        fputs("api-contract: null runtime tick accepted\n", stderr);
        return 1;
    }
    if (ghostty_gtk_embed_surface_new(NULL, NULL, NULL) != NULL) {
        fputs("api-contract: null runtime surface accepted\n", stderr);
        return 1;
    }
    ghostty_gtk_embed_surface_grab_focus(NULL);
    if (ghostty_gtk_embed_surface_send_text(NULL, "text") ||
        ghostty_gtk_embed_surface_request_paste(NULL)) {
        fputs("api-contract: null surface operation accepted\n", stderr);
        return 1;
    }

    puts("api-contract: PASS");
    return 0;
}
