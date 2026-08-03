#include <gtk/gtk.h>

/*
 * Minimal upstream reproducer for DOGFOOD-2026-08-03-MNEMONIC-LABEL-LEAK.
 *
 * This deliberately contains no Ghostty or Zentty code. GTK 4.14.5 loses
 * the GList allocated while updating the accessible labelled-by relation for
 * a mnemonic button. Keep this probe until the supported GTK baseline no
 * longer exhibits the finding or an upstream issue provides its replacement.
 */
int main(void) {
  gtk_init();

  GtkWidget *button = gtk_button_new_with_mnemonic("_Close");
  g_object_ref_sink(button);
  g_object_unref(button);

  while (g_main_context_iteration(NULL, FALSE)) {
  }
  return 0;
}
