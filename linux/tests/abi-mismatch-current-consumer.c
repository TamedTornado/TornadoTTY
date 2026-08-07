#include <ghostty/gtk.h>

#include <stddef.h>
#include <stdio.h>

int main(void) {
    puts("abi-mismatch-current-consumer: MAIN");
    ghostty_gtk_embed_surface_options_t options = {
        .struct_size = sizeof(options),
        .command = NULL,
        .title = NULL,
        .working_directory = NULL,
        .environment = NULL,
        .environment_count = 0,
    };
    if (ghostty_gtk_embed_surface_new_with_options(NULL, &options) != NULL) {
        fputs("abi-mismatch-current-consumer: null runtime accepted\n", stderr);
        return 1;
    }
    puts("abi-mismatch-current-consumer: PASS");
    return 0;
}
