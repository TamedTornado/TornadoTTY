#include <ghostty/gtk.h>
#include <gtk/gtk.h>

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    ghostty_gtk_embed_runtime_t *runtime;
    GtkApplication *application;
    GtkWindow *window;
    GtkWidget *terminal;
    guint tick_source;
    guint timeout_source;
    bool integration_test;
    bool activated;
    bool initialized;
    bool title_acknowledged;
    bool child_exited;
    bool tick_failed;
} ZenttyLinuxHost;

static gboolean tick_runtime(gpointer userdata) {
    ZenttyLinuxHost *host = userdata;
    if (!ghostty_gtk_embed_runtime_tick(host->runtime)) {
        host->tick_failed = true;
        g_printerr("zentty-linux: Ghostty runtime tick failed\n");
        return G_SOURCE_REMOVE;
    }

    return G_SOURCE_CONTINUE;
}

static gboolean integration_timeout(gpointer userdata) {
    ZenttyLinuxHost *host = userdata;
    host->timeout_source = 0;
    g_printerr("zentty-linux: integration timeout\n");
    g_application_quit(G_APPLICATION(host->application));
    return G_SOURCE_REMOVE;
}

static void terminal_initialized(GtkWidget *terminal, gpointer userdata) {
    (void) terminal;
    ZenttyLinuxHost *host = userdata;
    host->initialized = true;
    g_printerr("zentty-linux: terminal initialized\n");
}

static void terminal_title_changed(
    GObject *terminal,
    GParamSpec *property,
    gpointer userdata
) {
    (void) property;
    ZenttyLinuxHost *host = userdata;
    char *title = NULL;
    g_object_get(terminal, "title", &title, NULL);
    if (title != NULL && strcmp(title, "zentty-linux-integration") == 0) {
        host->title_acknowledged = true;
        g_printerr("zentty-linux: terminal output acknowledged\n");
    }
    g_free(title);
}

static void terminal_child_exited(
    GObject *terminal,
    GParamSpec *property,
    gpointer userdata
) {
    (void) terminal;
    (void) property;
    ZenttyLinuxHost *host = userdata;
    host->child_exited = true;
    g_printerr("zentty-linux: terminal child exited\n");
    if (host->integration_test) {
        if (host->timeout_source != 0) {
            g_source_remove(host->timeout_source);
            host->timeout_source = 0;
        }
        g_application_quit(G_APPLICATION(host->application));
    }
}

static void activate(GtkApplication *application, gpointer userdata) {
    ZenttyLinuxHost *host = userdata;
    host->activated = true;
    g_application_set_default(G_APPLICATION(application));

    host->window = GTK_WINDOW(gtk_application_window_new(application));
    gtk_window_set_title(host->window, "Zentty Linux");
    gtk_window_set_default_size(host->window, 1000, 700);

    const char *command = g_getenv("ZENTTY_LINUX_COMMAND");
    if (host->integration_test && command == NULL) {
        command = "printf '\\033]2;zentty-linux-integration\\a'; sleep 1";
    }

    host->terminal = ghostty_gtk_embed_surface_new(
        host->runtime,
        command,
        "Zentty Linux"
    );
    if (host->terminal == NULL) {
        g_printerr("zentty-linux: failed to create terminal widget\n");
        g_application_quit(G_APPLICATION(application));
        return;
    }

    g_signal_connect(
        host->terminal,
        "init",
        G_CALLBACK(terminal_initialized),
        host
    );
    g_signal_connect(
        host->terminal,
        "notify::title",
        G_CALLBACK(terminal_title_changed),
        host
    );
    g_signal_connect(
        host->terminal,
        "notify::child-exited",
        G_CALLBACK(terminal_child_exited),
        host
    );

    gtk_window_set_child(host->window, host->terminal);
    gtk_window_present(host->window);
    host->tick_source = g_timeout_add(1, tick_runtime, host);
    if (host->integration_test) {
        host->timeout_source = g_timeout_add_seconds(8, integration_timeout, host);
    }

    g_printerr("zentty-linux: activated\n");
}

static void destroy_host_window(ZenttyLinuxHost *host) {
    if (host->tick_source != 0) {
        g_source_remove(host->tick_source);
        host->tick_source = 0;
    }
    if (host->timeout_source != 0) {
        g_source_remove(host->timeout_source);
        host->timeout_source = 0;
    }
    if (host->window != NULL) {
        gtk_window_destroy(host->window);
        host->window = NULL;
        host->terminal = NULL;
    }
}

int main(int argc, char **argv) {
    ZenttyLinuxHost host = {0};
    host.integration_test = g_getenv("ZENTTY_LINUX_INTEGRATION_TEST") != NULL;
    const char *async_backend = g_getenv("ZENTTY_LINUX_ASYNC_BACKEND");
    if (async_backend != NULL && strcmp(async_backend, "epoll") == 0) {
        host.runtime = ghostty_gtk_embed_runtime_new_with_async_backend(
            GHOSTTY_GTK_EMBED_ASYNC_EPOLL
        );
    } else {
        host.runtime = ghostty_gtk_embed_runtime_new();
    }
    if (host.runtime == NULL) {
        g_printerr("zentty-linux: failed to initialize Ghostty runtime\n");
        return 1;
    }

    host.application = gtk_application_new(
        "com.tamedtornado.Zentty.Linux",
        G_APPLICATION_NON_UNIQUE
    );
    g_signal_connect(host.application, "activate", G_CALLBACK(activate), &host);

    const int application_status = g_application_run(
        G_APPLICATION(host.application),
        argc,
        argv
    );

    destroy_host_window(&host);
    g_object_unref(host.application);
    while (g_main_context_iteration(NULL, false)) {}
    ghostty_gtk_embed_runtime_free(host.runtime);

    if (host.integration_test) {
        const bool passed = host.activated &&
            host.initialized &&
            host.title_acknowledged &&
            host.child_exited &&
            !host.tick_failed;
        g_printerr(
            "zentty-linux: integration %s activated=%d initialized=%d title=%d child=%d tick_failed=%d\n",
            passed ? "PASS" : "FAIL",
            host.activated,
            host.initialized,
            host.title_acknowledged,
            host.child_exited,
            host.tick_failed
        );
        return passed ? application_status : 2;
    }

    return application_status;
}
