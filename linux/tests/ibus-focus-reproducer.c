#include <gtk/gtk.h>

#include <stdio.h>

#define ENTRY_COUNT 4

typedef struct {
    GtkApplication *application;
    GtkWidget *window;
    GtkWidget *entries[ENTRY_COUNT];
    GtkIMContext *context;
    guint focus_index;
} IBusFocusReproducer;

static gboolean advance_focus(gpointer userdata) {
    IBusFocusReproducer *reproducer = userdata;
    if (reproducer->focus_index < ENTRY_COUNT) {
        gtk_widget_grab_focus(
            reproducer->entries[reproducer->focus_index]
        );
        gtk_im_context_set_client_widget(
            reproducer->context,
            reproducer->entries[reproducer->focus_index]
        );
        gtk_im_context_focus_in(reproducer->context);
        gtk_im_context_focus_out(reproducer->context);
        reproducer->focus_index++;
        return G_SOURCE_CONTINUE;
    }

    gtk_im_context_set_client_widget(reproducer->context, NULL);
    gtk_window_destroy(GTK_WINDOW(reproducer->window));
    reproducer->window = NULL;
    g_object_unref(reproducer->context);
    reproducer->context = NULL;
    g_application_quit(G_APPLICATION(reproducer->application));
    return G_SOURCE_REMOVE;
}

static void activate(GtkApplication *application, gpointer userdata) {
    IBusFocusReproducer *reproducer = userdata;
    reproducer->window = gtk_application_window_new(application);
    reproducer->context = gtk_im_multicontext_new();
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    for (guint index = 0; index < ENTRY_COUNT; index++) {
        reproducer->entries[index] = gtk_entry_new();
        PangoContext *context = gtk_widget_get_pango_context(
            reproducer->entries[index]
        );
        PangoFontDescription *description =
            pango_font_description_from_string("monospace 12");
        PangoFontMetrics *metrics = pango_context_get_metrics(
            context,
            description,
            pango_context_get_language(context)
        );
        if (pango_font_metrics_get_height(metrics) <= 0) {
            g_error("Pango metrics did not produce a positive height");
        }
        pango_font_metrics_unref(metrics);
        pango_font_description_free(description);
        PangoLayout *layout = gtk_widget_create_pango_layout(
            reproducer->entries[index],
            "Zentty suppression governance"
        );
        int width = 0;
        int height = 0;
        pango_layout_get_size(layout, &width, &height);
        if (width <= 0 || height <= 0) {
            g_error("Pango layout did not produce a positive size");
        }
        g_object_unref(layout);
        gtk_box_append(GTK_BOX(box), reproducer->entries[index]);
    }
    gtk_window_set_child(GTK_WINDOW(reproducer->window), box);
    gtk_window_present(GTK_WINDOW(reproducer->window));
    g_timeout_add(50, advance_focus, reproducer);
}

int main(int argc, char **argv) {
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
    const int status = g_application_run(
        G_APPLICATION(reproducer.application),
        argc,
        argv
    );
    g_object_unref(reproducer.application);
    while (g_main_context_iteration(NULL, false)) {}
    if (status == 0) {
        puts("ibus-focus-reproducer: PASS");
    }
    return status;
}
