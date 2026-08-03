#include <gtk/gtk.h>

#include <stdio.h>

#define CLIENT_COUNT 4U
#define STATE_POLL_INTERVAL_MSEC 10U
#define LIFECYCLE_TIMEOUT_USEC (30 * G_USEC_PER_SEC)
#define CONTEXT_HANDLER_COUNT 6U

typedef enum {
    REPRODUCER_WAIT_WINDOW,
    REPRODUCER_WAIT_BASELINE_CONTEXT,
    REPRODUCER_ACQUIRE_FOCUS,
    REPRODUCER_WAIT_FOCUS,
    REPRODUCER_WAIT_IBUS_ACTIVE,
    REPRODUCER_QUERY_PREEDIT,
    REPRODUCER_WAIT_IBUS_INACTIVE,
    REPRODUCER_WAIT_TEARDOWN_BARRIER,
    REPRODUCER_DONE,
} IBusFocusState;

typedef struct {
    GtkApplication *application;
    GtkWidget *window;
    GtkWidget *clients[CLIENT_COUNT];
    GtkIMContext *context;
    GDBusConnection *ibus_bus;
    char *baseline_context_path;
    char *active_context_path;
    gulong context_handlers[CONTEXT_HANDLER_COUNT];
    guint source_id;
    guint focus_index;
    guint completed_cycles;
    guint late_callback_count;
    gint64 deadline_usec;
    IBusFocusState state;
    gboolean client_attached;
    gboolean focus_active;
    gboolean teardown_started;
    gboolean teardown_complete;
    gboolean failed;
} IBusFocusReproducer;

typedef enum {
    CONTEXT_ACK_WAIT,
    CONTEXT_ACK_ACCEPT,
    CONTEXT_ACK_REJECT,
} ContextAckResult;

static gboolean context_path_is_same(
    const char *expected,
    const char *actual
) {
    return expected != NULL && actual != NULL &&
        g_strcmp0(expected, actual) == 0;
}

static gboolean context_path_is_active(
    const char *baseline,
    const char *actual
) {
    return baseline != NULL && actual != NULL &&
        !context_path_is_same(baseline, actual);
}

static ContextAckResult classify_active_context(
    const char *baseline,
    const char *actual
) {
    if (baseline == NULL || actual == NULL) {
        return CONTEXT_ACK_REJECT;
    }
    if (context_path_is_same(baseline, actual)) {
        return CONTEXT_ACK_WAIT;
    }
    return CONTEXT_ACK_ACCEPT;
}

static ContextAckResult classify_inactive_context(
    const char *baseline,
    const char *active,
    const char *actual
) {
    if (baseline == NULL || active == NULL || actual == NULL) {
        return CONTEXT_ACK_REJECT;
    }
    if (context_path_is_same(active, actual)) {
        return CONTEXT_ACK_WAIT;
    }
    if (context_path_is_same(baseline, actual)) {
        return CONTEXT_ACK_ACCEPT;
    }
    return CONTEXT_ACK_REJECT;
}

static int context_path_contract_self_test(void) {
    const char *baseline = "/org/freedesktop/IBus/InputContext_1";
    const char *cycle_one = "/org/freedesktop/IBus/InputContext_2";
    const char *cycle_two = "/org/freedesktop/IBus/InputContext_3";

    if (!context_path_is_active(baseline, cycle_one) ||
        !context_path_is_same(cycle_one, cycle_one) ||
        !context_path_is_same(baseline, baseline) ||
        !context_path_is_active(baseline, cycle_two) ||
        !context_path_is_same(cycle_two, cycle_two) ||
        classify_active_context(baseline, baseline) != CONTEXT_ACK_WAIT ||
        classify_active_context(baseline, cycle_one) != CONTEXT_ACK_ACCEPT ||
        classify_active_context(NULL, cycle_one) != CONTEXT_ACK_REJECT ||
        classify_inactive_context(baseline, cycle_one, cycle_one) !=
            CONTEXT_ACK_WAIT ||
        classify_inactive_context(baseline, cycle_one, baseline) !=
            CONTEXT_ACK_ACCEPT ||
        classify_inactive_context(baseline, cycle_one, cycle_two) !=
            CONTEXT_ACK_REJECT ||
        context_path_is_active(baseline, baseline) ||
        context_path_is_same(cycle_one, cycle_two) ||
        context_path_is_same(cycle_one, baseline) ||
        context_path_is_same(NULL, cycle_one) ||
        context_path_is_active(NULL, cycle_one)) {
        fputs(
            "ibus-focus-reproducer: context-path contract self-test failed\n",
            stderr
        );
        return 1;
    }
    puts("ibus-focus-reproducer: context-path contract self-test passed");
    return 0;
}

static void fail_reproducer(
    IBusFocusReproducer *reproducer,
    const char *reason
) {
    if (!reproducer->failed) {
        reproducer->failed = true;
        fprintf(stderr, "ibus-focus-reproducer: error: %s\n", reason);
    }
    if (reproducer->application != NULL) {
        g_application_quit(G_APPLICATION(reproducer->application));
    }
}

static void observe_context_callback(IBusFocusReproducer *reproducer) {
    if (reproducer->teardown_started) {
        reproducer->late_callback_count++;
        fail_reproducer(reproducer, "late IM callback after final detach");
    }
}

static void context_event(GtkIMContext *context, gpointer userdata) {
    IBusFocusReproducer *reproducer = userdata;
    (void)context;
    observe_context_callback(reproducer);
}

static void context_commit(
    GtkIMContext *context,
    const char *text,
    gpointer userdata
) {
    IBusFocusReproducer *reproducer = userdata;
    (void)context;
    (void)text;
    observe_context_callback(reproducer);
}

static gboolean context_retrieve_surrounding(
    GtkIMContext *context,
    gpointer userdata
) {
    IBusFocusReproducer *reproducer = userdata;
    (void)context;
    observe_context_callback(reproducer);
    return false;
}

static gboolean context_delete_surrounding(
    GtkIMContext *context,
    int offset,
    int character_count,
    gpointer userdata
) {
    IBusFocusReproducer *reproducer = userdata;
    (void)context;
    (void)offset;
    (void)character_count;
    observe_context_callback(reproducer);
    return false;
}

static void disconnect_context_handlers(IBusFocusReproducer *reproducer) {
    if (reproducer->context == NULL) {
        return;
    }
    for (guint index = 0; index < CONTEXT_HANDLER_COUNT; index++) {
        if (reproducer->context_handlers[index] != 0U) {
            g_signal_handler_disconnect(
                reproducer->context,
                reproducer->context_handlers[index]
            );
            reproducer->context_handlers[index] = 0U;
        }
    }
}

static void release_context(IBusFocusReproducer *reproducer) {
    if (reproducer->context == NULL) {
        return;
    }
    if (reproducer->focus_active) {
        gtk_im_context_focus_out(reproducer->context);
        reproducer->focus_active = false;
    }
    gtk_im_context_reset(reproducer->context);
    if (reproducer->client_attached) {
        gtk_im_context_set_client_widget(reproducer->context, NULL);
        reproducer->client_attached = false;
    }
    disconnect_context_handlers(reproducer);
    g_object_unref(reproducer->context);
    reproducer->context = NULL;
}

static void cleanup_reproducer(IBusFocusReproducer *reproducer) {
    if (reproducer->source_id != 0U) {
        const guint source_id = reproducer->source_id;
        reproducer->source_id = 0U;
        g_source_remove(source_id);
    }
    release_context(reproducer);
    if (reproducer->window != NULL) {
        gtk_window_set_focus(GTK_WINDOW(reproducer->window), NULL);
        gtk_window_destroy(GTK_WINDOW(reproducer->window));
        reproducer->window = NULL;
    }
    g_clear_pointer(&reproducer->baseline_context_path, g_free);
    g_clear_pointer(&reproducer->active_context_path, g_free);
    g_clear_object(&reproducer->ibus_bus);
}

static gboolean current_input_context(
    IBusFocusReproducer *reproducer,
    char **path_out
) {
    GError *error = NULL;
    GVariant *reply = g_dbus_connection_call_sync(
        reproducer->ibus_bus,
        "org.freedesktop.IBus",
        "/org/freedesktop/IBus",
        "org.freedesktop.DBus.Properties",
        "Get",
        g_variant_new("(ss)", "org.freedesktop.IBus", "CurrentInputContext"),
        G_VARIANT_TYPE("(v)"),
        G_DBUS_CALL_FLAGS_NO_AUTO_START,
        1000,
        NULL,
        &error
    );
    if (reply == NULL) {
        if (error != NULL) {
            fprintf(
                stderr,
                "ibus-focus-reproducer: error: CurrentInputContext query "
                "failed: %s\n",
                error->message
            );
            g_error_free(error);
        } else {
            fprintf(
                stderr,
                "ibus-focus-reproducer: error: CurrentInputContext query "
                "failed without a GError\n"
            );
        }
        fail_reproducer(reproducer, "could not query the private IBus focus");
        return false;
    }

    GVariant *value = NULL;
    g_variant_get(reply, "(v)", &value);
    if (value == NULL ||
        !g_variant_is_of_type(value, G_VARIANT_TYPE_OBJECT_PATH)) {
        if (value != NULL) {
            g_variant_unref(value);
        }
        g_variant_unref(reply);
        fail_reproducer(
            reproducer,
            "CurrentInputContext did not return an object path"
        );
        return false;
    }

    *path_out = g_variant_dup_string(value, NULL);
    g_variant_unref(value);
    g_variant_unref(reply);
    if (*path_out == NULL || (*path_out)[0] == '\0') {
        g_clear_pointer(path_out, g_free);
        fail_reproducer(
            reproducer,
            "CurrentInputContext returned an empty object path"
        );
        return false;
    }
    return true;
}

static gboolean finish_teardown(gpointer userdata) {
    IBusFocusReproducer *reproducer = userdata;
    reproducer->source_id = 0U;

    if (reproducer->failed) {
        return G_SOURCE_REMOVE;
    }
    if (reproducer->state != REPRODUCER_WAIT_TEARDOWN_BARRIER ||
        !reproducer->teardown_started ||
        reproducer->context == NULL ||
        reproducer->window == NULL ||
        reproducer->focus_active ||
        reproducer->client_attached ||
        reproducer->completed_cycles != CLIENT_COUNT ||
        reproducer->late_callback_count != 0U) {
        fail_reproducer(reproducer, "invalid final teardown state");
        return G_SOURCE_REMOVE;
    }

    disconnect_context_handlers(reproducer);
    g_object_unref(reproducer->context);
    reproducer->context = NULL;
    gtk_window_set_focus(GTK_WINDOW(reproducer->window), NULL);
    gtk_window_destroy(GTK_WINDOW(reproducer->window));
    reproducer->window = NULL;
    reproducer->teardown_complete = true;
    reproducer->state = REPRODUCER_DONE;
    printf(
        "ibus-focus-reproducer: teardown=complete cycles=%u "
        "late-callbacks=%u\n",
        reproducer->completed_cycles,
        reproducer->late_callback_count
    );
    g_application_quit(G_APPLICATION(reproducer->application));
    return G_SOURCE_REMOVE;
}

static gboolean schedule_teardown_barrier(
    IBusFocusReproducer *reproducer
) {
    reproducer->teardown_started = true;
    reproducer->state = REPRODUCER_WAIT_TEARDOWN_BARRIER;
    reproducer->source_id = 0U;
    const guint barrier_id = g_idle_add_full(
        G_PRIORITY_LOW,
        finish_teardown,
        reproducer,
        NULL
    );
    if (barrier_id == 0U) {
        fail_reproducer(reproducer, "could not schedule teardown barrier");
        return false;
    }
    reproducer->source_id = barrier_id;
    return true;
}

static gboolean preedit_state_is_empty(GtkIMContext *context) {
    char *preedit = NULL;
    PangoAttrList *attributes = NULL;
    int cursor_position = -1;
    gtk_im_context_get_preedit_string(
        context,
        &preedit,
        &attributes,
        &cursor_position
    );

    gboolean valid = false;
    if (preedit != NULL && attributes != NULL) {
        PangoAttrList *empty_attributes = pango_attr_list_new();
        valid = preedit[0] == '\0' &&
            cursor_position == 0 &&
            pango_attr_list_equal(attributes, empty_attributes);
        pango_attr_list_unref(empty_attributes);
    }
    g_free(preedit);
    if (attributes != NULL) {
        pango_attr_list_unref(attributes);
    }
    return valid;
}

static gboolean drive_lifecycle(gpointer userdata) {
    IBusFocusReproducer *reproducer = userdata;
    if (reproducer->failed) {
        reproducer->source_id = 0U;
        return G_SOURCE_REMOVE;
    }
    if (g_get_monotonic_time() > reproducer->deadline_usec) {
        fail_reproducer(reproducer, "lifecycle readiness deadline exceeded");
        reproducer->source_id = 0U;
        return G_SOURCE_REMOVE;
    }

    switch (reproducer->state) {
        case REPRODUCER_WAIT_WINDOW:
            if (!gtk_widget_get_realized(reproducer->window) ||
                !gtk_widget_get_mapped(reproducer->window)) {
                return G_SOURCE_CONTINUE;
            }
            for (guint index = 0; index < CLIENT_COUNT; index++) {
                if (!gtk_widget_get_mapped(reproducer->clients[index])) {
                    return G_SOURCE_CONTINUE;
                }
            }
            printf(
                "ibus-focus-reproducer: window=mapped clients=%u\n",
                CLIENT_COUNT
            );
            reproducer->state = REPRODUCER_WAIT_BASELINE_CONTEXT;
            return G_SOURCE_CONTINUE;

        case REPRODUCER_WAIT_BASELINE_CONTEXT: {
            char *path = NULL;
            if (!current_input_context(reproducer, &path)) {
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            reproducer->baseline_context_path = path;
            puts("ibus-focus-reproducer: baseline-context=ready");
            reproducer->state = REPRODUCER_ACQUIRE_FOCUS;
            return G_SOURCE_CONTINUE;
        }

        case REPRODUCER_ACQUIRE_FOCUS: {
            if (reproducer->focus_index != reproducer->completed_cycles ||
                reproducer->focus_index >= CLIENT_COUNT ||
                reproducer->focus_active ||
                reproducer->client_attached) {
                fail_reproducer(reproducer, "invalid focus acquisition state");
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            GtkWidget *client = reproducer->clients[reproducer->focus_index];
            gtk_im_context_set_client_widget(reproducer->context, client);
            reproducer->client_attached = true;
            if (!gtk_widget_grab_focus(client)) {
                fail_reproducer(reproducer, "client refused keyboard focus");
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            reproducer->state = REPRODUCER_WAIT_FOCUS;
            return G_SOURCE_CONTINUE;
        }

        case REPRODUCER_WAIT_FOCUS: {
            GtkWidget *client = reproducer->clients[reproducer->focus_index];
            if (!gtk_widget_has_focus(client)) {
                return G_SOURCE_CONTINUE;
            }
            gtk_im_context_focus_in(reproducer->context);
            reproducer->focus_active = true;
            reproducer->state = REPRODUCER_WAIT_IBUS_ACTIVE;
            return G_SOURCE_CONTINUE;
        }

        case REPRODUCER_WAIT_IBUS_ACTIVE: {
            char *path = NULL;
            if (!current_input_context(reproducer, &path)) {
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            const ContextAckResult acknowledgement = classify_active_context(
                reproducer->baseline_context_path,
                path
            );
            if (acknowledgement == CONTEXT_ACK_WAIT) {
                g_free(path);
                return G_SOURCE_CONTINUE;
            }
            if (acknowledgement != CONTEXT_ACK_ACCEPT) {
                g_free(path);
                fail_reproducer(
                    reproducer,
                    "IBus returned an invalid active input context"
                );
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            if (reproducer->active_context_path == NULL) {
                reproducer->active_context_path = path;
            } else {
                const gboolean same_active_context = context_path_is_same(
                    reproducer->active_context_path,
                    path
                );
                g_free(path);
                if (!same_active_context) {
                    fail_reproducer(
                        reproducer,
                        "IBus changed the acknowledged GTK input context"
                    );
                    reproducer->source_id = 0U;
                    return G_SOURCE_REMOVE;
                }
            }
            printf(
                "ibus-focus-reproducer: cycle=%u ibus-context=active\n",
                reproducer->focus_index + 1U
            );
            reproducer->state = REPRODUCER_QUERY_PREEDIT;
            return G_SOURCE_CONTINUE;
        }

        case REPRODUCER_QUERY_PREEDIT: {
            char *path = NULL;
            if (!current_input_context(reproducer, &path)) {
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            const gboolean active_context_still_current =
                context_path_is_same(reproducer->active_context_path, path);
            g_free(path);
            if (!active_context_still_current) {
                fail_reproducer(
                    reproducer,
                    "IBus changed context after active acknowledgement"
                );
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            GtkWidget *client = reproducer->clients[reproducer->focus_index];
            if (!reproducer->focus_active ||
                !reproducer->client_attached ||
                !gtk_widget_has_focus(client)) {
                fail_reproducer(reproducer, "focus changed before preedit query");
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            if (!preedit_state_is_empty(reproducer->context)) {
                fail_reproducer(
                    reproducer,
                    "preedit state was not exactly empty with cursor zero"
                );
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }

            gtk_im_context_focus_out(reproducer->context);
            reproducer->focus_active = false;
            gtk_im_context_reset(reproducer->context);
            gtk_im_context_set_client_widget(reproducer->context, NULL);
            reproducer->client_attached = false;
            reproducer->completed_cycles++;
            printf(
                "ibus-focus-reproducer: cycle=%u focus=verified "
                "preedit=empty cursor=0 attributes=empty\n",
                reproducer->completed_cycles
            );
            reproducer->focus_index++;
            reproducer->state = REPRODUCER_WAIT_IBUS_INACTIVE;
            return G_SOURCE_CONTINUE;
        }

        case REPRODUCER_WAIT_IBUS_INACTIVE: {
            char *path = NULL;
            if (!current_input_context(reproducer, &path)) {
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            const ContextAckResult acknowledgement = classify_inactive_context(
                reproducer->baseline_context_path,
                reproducer->active_context_path,
                path
            );
            if (acknowledgement == CONTEXT_ACK_WAIT) {
                g_free(path);
                return G_SOURCE_CONTINUE;
            }
            g_free(path);
            if (acknowledgement != CONTEXT_ACK_ACCEPT) {
                fail_reproducer(
                    reproducer,
                    "IBus focus-out selected an unexpected input context"
                );
                reproducer->source_id = 0U;
                return G_SOURCE_REMOVE;
            }
            g_clear_pointer(&reproducer->active_context_path, g_free);
            printf(
                "ibus-focus-reproducer: cycle=%u ibus-context=inactive\n",
                reproducer->completed_cycles
            );
            if (reproducer->completed_cycles == CLIENT_COUNT) {
                if (!schedule_teardown_barrier(reproducer)) {
                    return G_SOURCE_REMOVE;
                }
                return G_SOURCE_REMOVE;
            }
            reproducer->state = REPRODUCER_ACQUIRE_FOCUS;
            return G_SOURCE_CONTINUE;
        }

        case REPRODUCER_WAIT_TEARDOWN_BARRIER:
        case REPRODUCER_DONE:
            fail_reproducer(reproducer, "state source ran after lifecycle completion");
            reproducer->source_id = 0U;
            return G_SOURCE_REMOVE;
    }

    fail_reproducer(reproducer, "unknown lifecycle state");
    reproducer->source_id = 0U;
    return G_SOURCE_REMOVE;
}

static gboolean configure_client(GtkWidget *client) {
    PangoContext *context = gtk_widget_get_pango_context(client);
    PangoFontDescription *description =
        pango_font_description_from_string("monospace 12");
    if (description == NULL) {
        return false;
    }
    PangoFontMetrics *metrics = pango_context_get_metrics(
        context,
        description,
        pango_context_get_language(context)
    );
    pango_font_description_free(description);
    if (metrics == NULL) {
        return false;
    }
    const int metrics_height = pango_font_metrics_get_height(metrics);
    pango_font_metrics_unref(metrics);
    if (metrics_height <= 0) {
        return false;
    }

    PangoLayout *layout = gtk_widget_create_pango_layout(
        client,
        "Zentty suppression governance"
    );
    if (layout == NULL) {
        return false;
    }
    int width = 0;
    int height = 0;
    pango_layout_get_size(layout, &width, &height);
    g_object_unref(layout);
    return width > 0 && height > 0;
}

static void activate(GtkApplication *application, gpointer userdata) {
    IBusFocusReproducer *reproducer = userdata;
    if (reproducer->window != NULL || reproducer->context != NULL ||
        reproducer->ibus_bus != NULL) {
        fail_reproducer(reproducer, "application activated more than once");
        return;
    }

    const char *ibus_address = g_getenv("IBUS_ADDRESS");
    if (ibus_address == NULL ||
        !g_str_has_prefix(ibus_address, "unix:path=")) {
        fail_reproducer(
            reproducer,
            "controlled IBus address is unavailable or not a Unix socket"
        );
        return;
    }
    GError *connection_error = NULL;
    reproducer->ibus_bus = g_dbus_connection_new_for_address_sync(
        ibus_address,
        G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
            G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
        NULL,
        NULL,
        &connection_error
    );
    if (reproducer->ibus_bus == NULL) {
        if (connection_error != NULL) {
            fprintf(
                stderr,
                "ibus-focus-reproducer: error: private IBus connection "
                "failed: %s\n",
                connection_error->message
            );
            g_error_free(connection_error);
        }
        fail_reproducer(reproducer, "could not connect to the private IBus");
        return;
    }

    reproducer->window = gtk_application_window_new(application);
    reproducer->context = gtk_im_multicontext_new();
    GtkIMMulticontext *multicontext =
        GTK_IM_MULTICONTEXT(reproducer->context);
    gtk_im_multicontext_set_context_id(multicontext, "ibus");
    const char *context_id = gtk_im_multicontext_get_context_id(multicontext);
    if (g_strcmp0(context_id, "ibus") != 0) {
        fail_reproducer(reproducer, "GTK did not select the requested IBus delegate");
        return;
    }
    const GType ibus_context_type = g_type_from_name("IBusIMContext");
    if (ibus_context_type == G_TYPE_INVALID ||
        !g_type_is_a(ibus_context_type, GTK_TYPE_IM_CONTEXT)) {
        fail_reproducer(reproducer, "GTK did not register IBusIMContext");
        return;
    }
    puts("ibus-focus-reproducer: delegate=ibus type=IBusIMContext");

    reproducer->context_handlers[0] = g_signal_connect(
        reproducer->context,
        "preedit-start",
        G_CALLBACK(context_event),
        reproducer
    );
    reproducer->context_handlers[1] = g_signal_connect(
        reproducer->context,
        "preedit-changed",
        G_CALLBACK(context_event),
        reproducer
    );
    reproducer->context_handlers[2] = g_signal_connect(
        reproducer->context,
        "preedit-end",
        G_CALLBACK(context_event),
        reproducer
    );
    reproducer->context_handlers[3] = g_signal_connect(
        reproducer->context,
        "commit",
        G_CALLBACK(context_commit),
        reproducer
    );
    reproducer->context_handlers[4] = g_signal_connect(
        reproducer->context,
        "retrieve-surrounding",
        G_CALLBACK(context_retrieve_surrounding),
        reproducer
    );
    reproducer->context_handlers[5] = g_signal_connect(
        reproducer->context,
        "delete-surrounding",
        G_CALLBACK(context_delete_surrounding),
        reproducer
    );

    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_window_set_default_size(GTK_WINDOW(reproducer->window), 400, 200);
    gtk_window_set_child(GTK_WINDOW(reproducer->window), box);
    for (guint index = 0; index < CLIENT_COUNT; index++) {
        GtkWidget *client = gtk_drawing_area_new();
        reproducer->clients[index] = client;
        gtk_widget_set_focusable(client, true);
        gtk_widget_set_hexpand(client, true);
        gtk_widget_set_size_request(client, 320, 32);
        gtk_box_append(GTK_BOX(box), client);
        if (!configure_client(client)) {
            fail_reproducer(reproducer, "Pango client proof failed");
            return;
        }
    }
    gtk_window_present(GTK_WINDOW(reproducer->window));

    reproducer->state = REPRODUCER_WAIT_WINDOW;
    reproducer->deadline_usec =
        g_get_monotonic_time() + LIFECYCLE_TIMEOUT_USEC;
    reproducer->source_id = g_timeout_add(
        STATE_POLL_INTERVAL_MSEC,
        drive_lifecycle,
        reproducer
    );
    if (reproducer->source_id == 0U) {
        fail_reproducer(reproducer, "could not schedule lifecycle state source");
    }
}

int main(int argc, char **argv) {
    if (argc == 2 &&
        g_strcmp0(argv[1], "--self-test-context-paths") == 0) {
        return context_path_contract_self_test();
    }
    IBusFocusReproducer reproducer = {0};
    reproducer.application = gtk_application_new(
        "com.tamedtornado.Zentty.IBusFocusReproducer",
        G_APPLICATION_NON_UNIQUE
    );
    g_signal_connect(
        reproducer.application,
        "activate",
        G_CALLBACK(activate),
        &reproducer
    );
    const int application_status = g_application_run(
        G_APPLICATION(reproducer.application),
        argc,
        argv
    );

    const gboolean lifecycle_complete =
        !reproducer.failed &&
        reproducer.state == REPRODUCER_DONE &&
        reproducer.completed_cycles == CLIENT_COUNT &&
        reproducer.focus_index == CLIENT_COUNT &&
        reproducer.source_id == 0U &&
        reproducer.context == NULL &&
        reproducer.window == NULL &&
        reproducer.teardown_complete &&
        reproducer.late_callback_count == 0U;
    if (application_status == 0 && !lifecycle_complete) {
        fail_reproducer(&reproducer, "application exited before lifecycle completion");
    }

    cleanup_reproducer(&reproducer);
    g_object_unref(reproducer.application);
    reproducer.application = NULL;
    if (application_status != 0) {
        return application_status;
    }
    if (!lifecycle_complete) {
        return 1;
    }
    puts("ibus-focus-reproducer: PASS");
    return 0;
}
