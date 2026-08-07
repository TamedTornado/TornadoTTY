#include <ghostty/gtk.h>
#include <gtk/gtk.h>

#include <stdio.h>
#include <string.h>

static int runtime_first(void) {
    ghostty_gtk_embed_runtime_t *runtime = ghostty_gtk_embed_runtime_new();
    if (runtime == NULL) {
        fputs("runtime-init-order: runtime-first constructor rejected\n", stderr);
        return 1;
    }

    gtk_init();
    GtkWidget *button = gtk_button_new_with_label("GTK remains usable");
    g_object_ref_sink(button);
    g_object_unref(button);
    ghostty_gtk_embed_runtime_free(runtime);
    puts("runtime-init-order: PASS runtime-first");
    return 0;
}

static int gtk_first(void) {
    gtk_init();
    GtkWidget *before = gtk_button_new_with_label("before rejected runtime");
    g_object_ref_sink(before);

    ghostty_gtk_embed_runtime_t *runtime = ghostty_gtk_embed_runtime_new();
    if (runtime != NULL) {
        ghostty_gtk_embed_runtime_free(runtime);
        g_object_unref(before);
        fputs("runtime-init-order: GTK-first constructor was accepted\n", stderr);
        return 1;
    }

    GtkWidget *after = gtk_button_new_with_label("after rejected runtime");
    g_object_ref_sink(after);
    while (g_main_context_iteration(NULL, false)) {}
    g_object_unref(after);
    g_object_unref(before);
    puts("runtime-init-order: PASS gtk-first-rejected-gtk-usable");
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 2) {
        fputs("usage: runtime-init-order runtime-first|gtk-first\n", stderr);
        return 64;
    }
    if (strcmp(argv[1], "runtime-first") == 0) {
        return runtime_first();
    }
    if (strcmp(argv[1], "gtk-first") == 0) {
        return gtk_first();
    }
    fputs("runtime-init-order: unknown order\n", stderr);
    return 64;
}
