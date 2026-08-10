#include <ghostty/gtk.h>
#include <gtk/gtk.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void count_text_callbacks(
    const char *text,
    size_t text_len,
    void *userdata
) {
    (void) text;
    (void) text_len;
    if (userdata != NULL) {
        int *count = userdata;
        *count += 1;
    }
}

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
    int text_callbacks = 0;
    ghostty_gtk_embed_cell_size_t cell_size = {.width = 17.0, .height = 23.0};
    if (ghostty_gtk_embed_surface_close(NULL) ||
        ghostty_gtk_embed_surface_close(foreign_surface) ||
        ghostty_gtk_embed_surface_binding_action(NULL, "start_search", 12) ||
        ghostty_gtk_embed_surface_binding_action(
            foreign_surface,
            "start_search",
            12
        ) ||
        ghostty_gtk_embed_surface_cell_size(NULL, &cell_size) ||
        ghostty_gtk_embed_surface_cell_size(foreign_surface, &cell_size) ||
        ghostty_gtk_embed_surface_send_text(NULL, "text") ||
        ghostty_gtk_embed_surface_send_text(foreign_surface, "text") ||
        ghostty_gtk_embed_surface_read_text(
            NULL,
            GHOSTTY_GTK_EMBED_TEXT_SCREEN,
            count_text_callbacks,
            &text_callbacks
        ) ||
        ghostty_gtk_embed_surface_read_text(
            foreign_surface,
            GHOSTTY_GTK_EMBED_TEXT_SCREEN,
            count_text_callbacks,
            &text_callbacks
        ) ||
        ghostty_gtk_embed_surface_read_selection(
            NULL,
            count_text_callbacks,
            &text_callbacks
        ) ||
        ghostty_gtk_embed_surface_read_selection(
            foreign_surface,
            count_text_callbacks,
            &text_callbacks
        ) ||
        ghostty_gtk_embed_surface_request_paste(NULL) ||
        ghostty_gtk_embed_surface_request_paste(foreign_surface)) {
        fputs("api-contract: null or foreign surface accepted\n", stderr);
        return 1;
    }
    if (text_callbacks != 0) {
        fputs("api-contract: rejected text read invoked callback\n", stderr);
        return 1;
    }
    if (cell_size.width != 17.0 || cell_size.height != 23.0) {
        fputs("api-contract: rejected cell-size call mutated output\n", stderr);
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

    ghostty_gtk_embed_surface_options_t legacy_sized = {
        .struct_size = offsetof(
            ghostty_gtk_embed_surface_options_t,
            environment
        ),
        .command = "exit 0",
        .title = "legacy size contract",
        .working_directory = "/tmp",
        .environment = NULL,
        .environment_count = 129,
    };
    GtkWidget *legacy_surface =
        ghostty_gtk_embed_surface_new_with_options(runtime, &legacy_sized);
    if (legacy_surface == NULL) {
        fputs("api-contract: old-sized surface options rejected\n", stderr);
        return 1;
    }
    g_object_ref_sink(legacy_surface);
    if (!ghostty_gtk_embed_surface_close(legacy_surface)) {
        fputs("api-contract: old-sized surface close rejected\n", stderr);
        return 1;
    }
    g_object_unref(legacy_surface);

    const char *environment[] = {"ZENTTY_API_ENV=present"};

    ghostty_gtk_embed_surface_options_t options = {
        .struct_size = sizeof(options),
        .command = "exit 0",
        .title = "API options contract",
        .working_directory = "/tmp",
        .environment = environment,
        .environment_count = 1,
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

    ghostty_gtk_embed_surface_options_t invalid_environment = options;
    invalid_environment.environment = NULL;
    if (ghostty_gtk_embed_surface_new_with_options(
            runtime,
            &invalid_environment
        ) != NULL) {
        fputs("api-contract: null environment array accepted\n", stderr);
        return 1;
    }
    const char *null_environment[] = {NULL};
    invalid_environment.environment = null_environment;
    if (ghostty_gtk_embed_surface_new_with_options(
            runtime,
            &invalid_environment
        ) != NULL) {
        fputs("api-contract: null environment entry accepted\n", stderr);
        return 1;
    }
    const char *malformed_environment[] = {"MISSING_EQUALS"};
    invalid_environment.environment = malformed_environment;
    if (ghostty_gtk_embed_surface_new_with_options(
            runtime,
            &invalid_environment
        ) != NULL) {
        fputs("api-contract: malformed environment entry accepted\n", stderr);
        return 1;
    }
    invalid_environment.environment = environment;
    invalid_environment.environment_count = 129;
    if (ghostty_gtk_embed_surface_new_with_options(
            runtime,
            &invalid_environment
        ) != NULL) {
        fputs("api-contract: excessive environment count accepted\n", stderr);
        return 1;
    }

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
    int text_callbacks = 0;
    ghostty_gtk_embed_cell_size_t cell_size = {.width = 17.0, .height = 23.0};
    if (g_signal_lookup("progress-report", G_OBJECT_TYPE(surface)) == 0) {
        fputs("api-contract: progress-report signal is missing\n", stderr);
        return 1;
    }
    if (ghostty_gtk_embed_surface_send_text(surface, NULL) ||
        ghostty_gtk_embed_surface_send_text(surface, "before-init") ||
        ghostty_gtk_embed_surface_binding_action(surface, "start_search", 12) ||
        ghostty_gtk_embed_surface_cell_size(surface, NULL) ||
        ghostty_gtk_embed_surface_cell_size(surface, &cell_size) ||
        ghostty_gtk_embed_surface_read_text(
            surface,
            GHOSTTY_GTK_EMBED_TEXT_SCREEN,
            count_text_callbacks,
            NULL
        ) ||
        ghostty_gtk_embed_surface_read_selection(
            surface,
            count_text_callbacks,
            &text_callbacks
        ) ||
        ghostty_gtk_embed_surface_read_selection(surface, NULL, NULL) ||
        ghostty_gtk_embed_surface_read_text(
            surface,
            (ghostty_gtk_embed_text_extent_t) 999,
            count_text_callbacks,
            NULL
        ) ||
        ghostty_gtk_embed_surface_read_text(
            surface,
            GHOSTTY_GTK_EMBED_TEXT_SCREEN,
            NULL,
            NULL
        ) ||
        ghostty_gtk_embed_surface_request_paste(surface)) {
        fputs("api-contract: uninitialized surface operation accepted\n", stderr);
        return 1;
    }
    if (cell_size.width != 17.0 || cell_size.height != 23.0) {
        fputs("api-contract: uninitialized cell-size call mutated output\n", stderr);
        return 1;
    }
    if (text_callbacks != 0) {
        fputs("api-contract: uninitialized selection read invoked callback\n", stderr);
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
