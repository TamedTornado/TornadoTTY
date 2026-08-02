#include <ghostty/gtk.h>
#include <gtk/gtk.h>

#include "host_options.h"

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#define ZENTTY_MAX_TERMINALS 4

typedef struct ZenttyLinuxHost ZenttyLinuxHost;

typedef struct {
    ZenttyLinuxHost *host;
    GtkWidget *widget;
    guint index;
    bool title_acknowledged;
    bool child_exited;
    bool initialized;
} ZenttyTerminal;

struct ZenttyLinuxHost {
    ghostty_gtk_embed_runtime_t *runtime;
    GtkApplication *application;
    GtkWindow *window;
    GtkWidget *grid;
    ZenttyTerminal terminals[ZENTTY_MAX_TERMINALS];
    guint terminal_count;
    guint initialized_count;
    guint title_count;
    guint child_exited_count;
    guint focus_index;
    guint focus_confirmed;
    int resize_width_before;
    guint interaction_phase;
    guint tick_source;
    guint qualification_source;
    guint timeout_source;
    bool integration_test;
    bool interaction_test;
    bool external_resize_test;
    bool physical_key_test;
    bool activated;
    bool interaction_started;
    bool keyboard_sent;
    bool clipboard_write;
    bool clipboard_read;
    bool resize_observed;
    bool interaction_qualified;
    bool tick_failed;
};

static void quit_integration_when_complete(ZenttyLinuxHost *host) {
    if (!host->integration_test ||
        host->child_exited_count != host->terminal_count ||
        !host->interaction_qualified) {
        return;
    }

    if (host->timeout_source != 0) {
        g_source_remove(host->timeout_source);
        host->timeout_source = 0;
    }
    g_application_quit(G_APPLICATION(host->application));
}

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
    ZenttyTerminal *slot = userdata;
    if (slot->initialized) {
        return;
    }
    slot->initialized = true;
    slot->host->initialized_count++;
    g_printerr(
        "zentty-linux: terminal initialized index=%u count=%u\n",
        slot->index,
        slot->host->initialized_count
    );
}

static void terminal_title_changed(
    GObject *terminal,
    GParamSpec *property,
    gpointer userdata
) {
    (void) property;
    ZenttyTerminal *slot = userdata;
    char *title = NULL;
    char *expected = NULL;
    if (slot->host->physical_key_test) {
        expected = g_strdup("zentty-linux-physical-key-ack");
    } else if (slot->host->interaction_test && slot->index == 0) {
        expected = g_strdup("zentty-linux-keyboard-ack");
    } else if (slot->host->interaction_test && slot->index == 1) {
        expected = g_strdup("zentty-linux-clipboard-ack");
    } else {
        expected = g_strdup_printf(
            "zentty-linux-integration-%u",
            slot->index
        );
    }
    g_object_get(terminal, "title", &title, NULL);
    if (!slot->title_acknowledged &&
        title != NULL && strcmp(title, expected) == 0) {
        slot->title_acknowledged = true;
        slot->host->title_count++;
        g_printerr(
            "zentty-linux: terminal output acknowledged index=%u count=%u\n",
            slot->index,
            slot->host->title_count
        );
    }
    g_free(expected);
    g_free(title);
}

static void terminal_clipboard_write(
    GtkWidget *terminal,
    int clipboard_type,
    const char *text,
    gpointer userdata
) {
    ZenttyTerminal *slot = userdata;
    if (!slot->host->interaction_test || slot->host->clipboard_write ||
        slot->index != 2 || text == NULL || clipboard_type != 0 ||
        strcmp(text, "zentty-product-clipboard") != 0) {
        return;
    }

    gdk_clipboard_set_text(gtk_widget_get_clipboard(terminal), text);
    slot->host->clipboard_write = true;
    if (!ghostty_gtk_embed_surface_request_paste(
            slot->host->terminals[1].widget)) {
        g_printerr("zentty-linux: clipboard paste request failed\n");
        return;
    }
    g_printerr("zentty-linux: clipboard write and paste request observed\n");
}

static void terminal_clipboard_read(GtkWidget *terminal, gpointer userdata) {
    (void) terminal;
    ZenttyTerminal *slot = userdata;
    if (!slot->host->interaction_test || slot->host->clipboard_read ||
        slot->index != 1) {
        return;
    }

    slot->host->clipboard_read = true;
    if (!ghostty_gtk_embed_surface_send_text(slot->widget, "\r")) {
        g_printerr("zentty-linux: clipboard Enter injection failed\n");
        return;
    }
    g_printerr("zentty-linux: clipboard read observed\n");
}

static void terminal_child_exited(
    GObject *terminal,
    GParamSpec *property,
    gpointer userdata
) {
    (void) terminal;
    (void) property;
    ZenttyTerminal *slot = userdata;
    if (slot->child_exited) {
        return;
    }
    slot->child_exited = true;
    slot->host->child_exited_count++;
    g_printerr(
        "zentty-linux: terminal child exited index=%u count=%u\n",
        slot->index,
        slot->host->child_exited_count
    );
    quit_integration_when_complete(slot->host);
}

static bool terminal_contains_focus(const ZenttyTerminal *slot) {
    GtkWidget *focused = gtk_root_get_focus(GTK_ROOT(slot->host->window));
    return focused != NULL &&
        (focused == slot->widget ||
            gtk_widget_is_ancestor(focused, slot->widget));
}

static gboolean qualify_physical_key(gpointer userdata) {
    ZenttyLinuxHost *host = userdata;
    ZenttyTerminal *slot = &host->terminals[0];
    if (!slot->initialized || !terminal_contains_focus(slot)) {
        ghostty_gtk_embed_surface_grab_focus(slot->widget);
        return G_SOURCE_CONTINUE;
    }

    host->qualification_source = 0;
    g_printerr("zentty-linux: physical key input ready\n");
    return G_SOURCE_REMOVE;
}

static gboolean qualify_multi_terminal(gpointer userdata) {
    ZenttyLinuxHost *host = userdata;
    if (host->initialized_count != host->terminal_count) {
        return G_SOURCE_CONTINUE;
    }

    if (host->interaction_test && !host->interaction_started) {
        host->interaction_started = true;
        const bool text_sent = ghostty_gtk_embed_surface_send_text(
            host->terminals[0].widget,
            "zentty-product-keyboard"
        );
        const bool enter_sent = ghostty_gtk_embed_surface_send_text(
            host->terminals[0].widget,
            "\r"
        );
        host->keyboard_sent = text_sent && enter_sent;
        g_printerr(
            "zentty-linux: keyboard input sent=%d\n",
            host->keyboard_sent
        );
        return G_SOURCE_CONTINUE;
    }

    if (host->interaction_phase == 0) {
        ZenttyTerminal *slot = &host->terminals[host->focus_index];
        if (!terminal_contains_focus(slot)) {
            ghostty_gtk_embed_surface_grab_focus(slot->widget);
            return G_SOURCE_CONTINUE;
        }

        host->focus_confirmed++;
        host->focus_index++;
        if (host->focus_index < host->terminal_count) {
            ghostty_gtk_embed_surface_grab_focus(
                host->terminals[host->focus_index].widget
            );
            return G_SOURCE_CONTINUE;
        }

        g_printerr(
            "zentty-linux: focus transitions confirmed=%u\n",
            host->focus_confirmed
        );
        host->resize_width_before = gtk_widget_get_width(
            host->terminals[0].widget
        );
        host->interaction_phase = 1;
        return G_SOURCE_CONTINUE;
    }

    if (host->interaction_phase == 1) {
        if (host->resize_width_before <= 0) {
            host->resize_width_before = gtk_widget_get_width(
                host->terminals[0].widget
            );
            return G_SOURCE_CONTINUE;
        }
        if (host->external_resize_test) {
            g_printerr(
                "zentty-linux: external resize ready width=%d\n",
                host->resize_width_before
            );
            host->interaction_phase = 2;
            return G_SOURCE_CONTINUE;
        }
        for (guint index = 0; index < host->terminal_count; index++) {
            g_object_ref(host->terminals[index].widget);
            gtk_grid_remove(
                GTK_GRID(host->grid),
                host->terminals[index].widget
            );
        }
        for (guint index = 0; index < host->terminal_count; index++) {
            gtk_grid_attach(
                GTK_GRID(host->grid),
                host->terminals[index].widget,
                0,
                (int) index,
                1,
                1
            );
            g_object_unref(host->terminals[index].widget);
        }
        host->interaction_phase = 2;
        return G_SOURCE_CONTINUE;
    }

    const int width = gtk_widget_get_width(host->terminals[0].widget);
    if (width == host->resize_width_before) {
        return G_SOURCE_CONTINUE;
    }

    host->resize_observed = true;
    host->interaction_qualified = true;
    host->qualification_source = 0;
    g_printerr(
        "zentty-linux: resize observed before=%d after=%d\n",
        host->resize_width_before,
        width
    );
    quit_integration_when_complete(host);
    return G_SOURCE_REMOVE;
}

static void activate(GtkApplication *application, gpointer userdata) {
    ZenttyLinuxHost *host = userdata;
    host->activated = true;
    g_application_set_default(G_APPLICATION(application));

    host->window = GTK_WINDOW(gtk_application_window_new(application));
    gtk_window_set_title(host->window, "Zentty Linux");
    gtk_window_set_default_size(host->window, 1000, 700);
    host->grid = gtk_grid_new();
    gtk_grid_set_row_homogeneous(GTK_GRID(host->grid), true);
    gtk_grid_set_column_homogeneous(GTK_GRID(host->grid), true);

    const char *configured_command = g_getenv("ZENTTY_LINUX_COMMAND");
    for (guint index = 0; index < host->terminal_count; index++) {
        ZenttyTerminal *slot = &host->terminals[index];
        slot->host = host;
        slot->index = index;

        char *integration_command = NULL;
        const char *command = configured_command;
        if (host->physical_key_test && command == NULL) {
            integration_command = g_strdup(
                "IFS= read -r value; "
                "[ \"$value\" = zentty-physical-key ] && "
                "printf '\033]2;zentty-linux-physical-key-ack\a'; sleep 1"
            );
            command = integration_command;
        } else if (host->interaction_test && command == NULL && index == 0) {
            integration_command = g_strdup(
                "IFS= read -r value; "
                "[ \"$value\" = zentty-product-keyboard ] && "
                "printf '\\033]2;zentty-linux-keyboard-ack\\a'; sleep 4"
            );
            command = integration_command;
        } else if (host->interaction_test && command == NULL && index == 1) {
            integration_command = g_strdup(
                "IFS= read -r value; "
                "[ \"$value\" = zentty-product-clipboard ] && "
                "printf '\\033]2;zentty-linux-clipboard-ack\\a'; sleep 4"
            );
            command = integration_command;
        } else if (host->interaction_test && command == NULL && index == 2) {
            integration_command = g_strdup(
                "printf '\\033]52;c;emVudHR5LXByb2R1Y3QtY2xpcGJvYXJk\\a'; "
                "printf '\\033]2;zentty-linux-integration-2\\a'; sleep 4"
            );
            command = integration_command;
        } else if (host->integration_test && command == NULL) {
            integration_command = g_strdup_printf(
                "printf '\\033]2;zentty-linux-integration-%u\\a'; sleep %u",
                index,
                host->terminal_count > 1 ? 4 : 1
            );
            command = integration_command;
        }

        slot->widget = ghostty_gtk_embed_surface_new(
            host->runtime,
            command,
            "Zentty Linux"
        );
        g_free(integration_command);
        if (slot->widget == NULL) {
            g_printerr(
                "zentty-linux: failed to create terminal widget index=%u\n",
                index
            );
            g_application_quit(G_APPLICATION(application));
            return;
        }

        g_signal_connect(
            slot->widget,
            "init",
            G_CALLBACK(terminal_initialized),
            slot
        );
        g_signal_connect(
            slot->widget,
            "clipboard-write",
            G_CALLBACK(terminal_clipboard_write),
            slot
        );
        g_signal_connect(
            slot->widget,
            "clipboard-read",
            G_CALLBACK(terminal_clipboard_read),
            slot
        );
        g_signal_connect(
            slot->widget,
            "notify::title",
            G_CALLBACK(terminal_title_changed),
            slot
        );
        g_signal_connect(
            slot->widget,
            "notify::child-exited",
            G_CALLBACK(terminal_child_exited),
            slot
        );
        gtk_grid_attach(
            GTK_GRID(host->grid),
            slot->widget,
            (int) (index % 2),
            (int) (index / 2),
            1,
            1
        );
    }

    gtk_window_set_child(host->window, host->grid);
    gtk_window_present(host->window);
    host->tick_source = g_timeout_add(1, tick_runtime, host);
    if (host->integration_test) {
        if (host->terminal_count == 1) {
            host->interaction_qualified = true;
            if (host->physical_key_test) {
                host->qualification_source = g_timeout_add(
                    50,
                    qualify_physical_key,
                    host
                );
            }
        } else {
            host->qualification_source = g_timeout_add(
                50,
                qualify_multi_terminal,
                host
            );
        }
        host->timeout_source = g_timeout_add_seconds(
            g_getenv("ZENTTY_LINUX_SLOW_INTEGRATION_TEST") != NULL
                ? 240
                : (host->terminal_count > 1 ? 12 : 8),
            integration_timeout,
            host
        );
    }

    g_printerr("zentty-linux: activated surfaces=%u\n", host->terminal_count);
}

static void destroy_host_window(ZenttyLinuxHost *host) {
    if (host->tick_source != 0) {
        g_source_remove(host->tick_source);
        host->tick_source = 0;
    }
    if (host->qualification_source != 0) {
        g_source_remove(host->qualification_source);
        host->qualification_source = 0;
    }
    if (host->timeout_source != 0) {
        g_source_remove(host->timeout_source);
        host->timeout_source = 0;
    }
    if (host->window != NULL) {
        gtk_window_destroy(host->window);
        host->window = NULL;
        host->grid = NULL;
        for (guint index = 0;
            index < host->terminal_count && index < ZENTTY_MAX_TERMINALS;
            index++) {
            host->terminals[index].widget = NULL;
        }
    }
}

int main(int argc, char **argv) {
    ZenttyLinuxHost host = {0};
    host.interaction_test =
        g_getenv("ZENTTY_LINUX_INTERACTION_TEST") != NULL;
    host.external_resize_test =
        g_getenv("ZENTTY_LINUX_EXTERNAL_RESIZE_TEST") != NULL;
    host.physical_key_test =
        g_getenv("ZENTTY_LINUX_PHYSICAL_KEY_TEST") != NULL;
    host.integration_test = host.interaction_test || host.external_resize_test ||
        host.physical_key_test ||
        g_getenv("ZENTTY_LINUX_INTEGRATION_TEST") != NULL;
    unsigned int terminal_count = 1;
    if (host.interaction_test || host.external_resize_test) {
        terminal_count = 4;
    } else if (host.integration_test &&
        !zentty_parse_terminal_count(
            g_getenv("ZENTTY_LINUX_INTEGRATION_SURFACES"),
            &terminal_count
        )) {
        g_printerr("zentty-linux: invalid integration surface count\n");
        return 64;
    }
    host.terminal_count = terminal_count;

    const char *async_backend = g_getenv("ZENTTY_LINUX_ASYNC_BACKEND");
    ghostty_gtk_embed_async_backend_t selected_backend;
    if (!zentty_parse_async_backend(async_backend, &selected_backend)) {
        g_printerr("zentty-linux: invalid async backend\n");
        return 64;
    }
    host.runtime = ghostty_gtk_embed_runtime_new_with_async_backend(
        selected_backend
    );
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
            host.initialized_count == host.terminal_count &&
            host.title_count == host.terminal_count &&
            host.child_exited_count == host.terminal_count &&
            host.interaction_qualified &&
            (!host.interaction_test ||
                (host.keyboard_sent &&
                    host.clipboard_write &&
                    host.clipboard_read)) &&
            !host.tick_failed;
        g_printerr(
            "zentty-linux: integration %s surfaces=%u initialized=%u titles=%u children=%u focus=%u resize=%d keyboard=%d clipboard=%d/%d tick_failed=%d\n",
            passed ? "PASS" : "FAIL",
            host.terminal_count,
            host.initialized_count,
            host.title_count,
            host.child_exited_count,
            host.focus_confirmed,
            host.resize_observed,
            host.keyboard_sent,
            host.clipboard_write,
            host.clipboard_read,
            host.tick_failed
        );
        return passed ? application_status : 2;
    }

    return application_status;
}
