#include <ghostty/gtk.h>

#include <stddef.h>
#include <stdio.h>

int main(void) {
    puts("abi-mismatch-historical-consumer: MAIN");
    if (ghostty_gtk_embed_surface_new(NULL, NULL, NULL) != NULL) {
        fputs("abi-mismatch-historical-consumer: null runtime accepted\n", stderr);
        return 1;
    }
    puts("abi-mismatch-historical-consumer: PASS");
    return 0;
}
