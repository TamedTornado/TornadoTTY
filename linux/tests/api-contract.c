#include <ghostty/gtk.h>
#include <gtk/gtk.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int reject_null_and_foreign_handles(void) {
    ghostty_gtk_embed_runtime_t *foreign_runtime =
        (ghostty_gtk_embed_runtime_t *) gtk_button_new();
    GtkWidget *foreign_surface = GTK_WIDGET(foreign_runtime);

    ghostty_gtk_embed_runtime_free(NULL);
    ghostty_gtk_embed_runtime_free(foreign_runtime);
    if (ghostty_gtk_embed_runtime_tick(NULL) ||
        ghostty_gtk_embed_runtime_tick(foreign_runtime) ||
        ghostty_gtk_embed_surface_new(NULL, NULL, NULL) != NULL ||
        ghostty_gtk_embed_surface_new_with_options(NULL, NULL) != NULL ||
        ghostty_gtk_embed_surface_new(foreign_runtime, NULL, NULL) != NULL) {
        fputs("api-contract: null or foreign runtime accepted\n", stderr);
        return 1;
    }

    ghostty_gtk_embed_surface_grab_focus(NULL);
    ghostty_gtk_embed_surface_grab_focus(foreign_surface);
    if (ghostty_gtk_embed_surface_close(NULL) ||
        ghostty_gtk_embed_surface_close(foreign_surface) ||
        ghostty_gtk_embed_surface_binding_action(NULL, "start_search", 12) ||
        ghostty_gtk_embed_surface_binding_action(
            foreign_surface,
            "start_search",
            12
        ) ||
        ghostty_gtk_embed_surface_send_text(NULL, "text") ||
        ghostty_gtk_embed_surface_send_text(foreign_surface, "text") ||
        ghostty_gtk_embed_surface_request_paste(NULL) ||
        ghostty_gtk_embed_surface_request_paste(foreign_surface)) {
        fputs("api-contract: null or foreign surface accepted\n", stderr);
        return 1;
    }

    g_object_ref_sink(foreign_surface);
    g_object_unref(foreign_surface);
    return 0;
}

static int enforce_runtime_lifecycle(
    ghostty_gtk_embed_runtime_t *runtime
) {
    if (ghostty_gtk_embed_runtime_new() != NULL) {
        fputs("api-contract: concurrent runtime accepted\n", stderr);
        return 1;
    }
    if (!ghostty_gtk_embed_runtime_tick(runtime)) {
        fputs("api-contract: active runtime tick rejected\n", stderr);
        return 1;
    }

    ghostty_gtk_embed_surface_options_t truncated = {
        .struct_size = offsetof(
            ghostty_gtk_embed_surface_options_t,
            working_directory
        ),
        .working_directory = "/tmp",
    };
    if (ghostty_gtk_embed_surface_new_with_options(runtime, NULL) != NULL ||
        ghostty_gtk_embed_surface_new_with_options(runtime, &truncated) != NULL) {
        fputs("api-contract: invalid surface options accepted\n", stderr);
        return 1;
    }

    ghostty_gtk_embed_surface_options_t options = {
        .struct_size = sizeof(options),
        .command = "exit 0",
        .title = "API options contract",
        .working_directory = "/tmp",
    };
    GtkWidget *options_surface =
        ghostty_gtk_embed_surface_new_with_options(runtime, &options);
    if (options_surface == NULL) {
        fputs("api-contract: valid surface options rejected\n", stderr);
        return 1;
    }
    g_object_ref_sink(options_surface);
    if (!ghostty_gtk_embed_surface_close(options_surface)) {
        fputs("api-contract: options surface close rejected\n", stderr);
        return 1;
    }
    g_object_unref(options_surface);

    GtkWidget *surface = ghostty_gtk_embed_surface_new(
        runtime,
        "exit 0",
        "API contract"
    );
    if (surface == NULL) {
        fputs("api-contract: active runtime surface rejected\n", stderr);
        return 1;
    }
    g_object_ref_sink(surface);
    if (ghostty_gtk_embed_surface_send_text(surface, NULL) ||
        ghostty_gtk_embed_surface_send_text(surface, "before-init") ||
        ghostty_gtk_embed_surface_binding_action(surface, "start_search", 12) ||
        ghostty_gtk_embed_surface_request_paste(surface)) {
        fputs("api-contract: uninitialized surface operation accepted\n", stderr);
        return 1;
    }
    ghostty_gtk_embed_surface_grab_focus(surface);
    if (!ghostty_gtk_embed_surface_close(surface)) {
        fputs("api-contract: uninitialized surface close rejected\n", stderr);
        return 1;
    }
    g_object_unref(surface);
    while (g_main_context_iteration(NULL, false)) {}

    ghostty_gtk_embed_runtime_free(runtime);
    ghostty_gtk_embed_runtime_free(runtime);
    if (ghostty_gtk_embed_runtime_tick(runtime) ||
        ghostty_gtk_embed_surface_new(runtime, NULL, NULL) != NULL) {
        fputs("api-contract: stale runtime accepted\n", stderr);
        return 1;
    }

    runtime = ghostty_gtk_embed_runtime_new();
    if (runtime != NULL) {
        fputs("api-contract: runtime recreation accepted\n", stderr);
        return 1;
    }
    return 0;
}

int main(void) {
    if (ghostty_gtk_embed_runtime_new_with_async_backend(
            (ghostty_gtk_embed_async_backend_t) 999) != NULL) {
        fputs("api-contract: invalid async backend accepted\n", stderr);
        return 1;
    }
    const char *backend = getenv("ZENTTY_API_CONTRACT_ASYNC_BACKEND");
    ghostty_gtk_embed_async_backend_t selected_backend =
        GHOSTTY_GTK_EMBED_ASYNC_DEFAULT;
    if (backend != NULL && strcmp(backend, "epoll") == 0) {
        selected_backend = GHOSTTY_GTK_EMBED_ASYNC_EPOLL;
    } else if (backend != NULL && strcmp(backend, "io_uring") == 0) {
        selected_backend = GHOSTTY_GTK_EMBED_ASYNC_IO_URING;
    } else if (backend != NULL && strcmp(backend, "default") != 0) {
        fputs("api-contract: unknown selected async backend\n", stderr);
        return 64;
    }
    ghostty_gtk_embed_runtime_t *runtime =
        ghostty_gtk_embed_runtime_new_with_async_backend(selected_backend);
    if (runtime == NULL) {
        fputs("api-contract: valid runtime rejected\n", stderr);
        return 1;
    }

    gtk_init();
    if (reject_null_and_foreign_handles() != 0 ||
        enforce_runtime_lifecycle(runtime) != 0) {
        return 1;
    }

    puts("api-contract: PASS");
    return 0;
}
