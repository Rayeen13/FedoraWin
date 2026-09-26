/* A real GTK4/libadwaita window on GTK's Win32 backend.
 * FedoraWin owns this HWND. Never inject or subclass foreign applications. */
#include <adwaita.h>

static void activate(GtkApplication *application, gpointer unused) {
  (void)unused;
  const char *theme = g_getenv("FEDORAWIN_ADWAITA_THEME");
  AdwStyleManager *style = adw_style_manager_get_default();
  if (g_strcmp0(theme, "dark") == 0)
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_FORCE_DARK);
  else if (g_strcmp0(theme, "light") == 0)
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_FORCE_LIGHT);
  else
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_DEFAULT);

  GtkWidget *window = adw_application_window_new(application);
  gtk_window_set_title(GTK_WINDOW(window), "FedoraWin Adwaita Native Probe");
  gtk_window_set_default_size(GTK_WINDOW(window), 800, 520);
  AdwToolbarView *toolbar = ADW_TOOLBAR_VIEW(adw_toolbar_view_new());
  AdwHeaderBar *header = ADW_HEADER_BAR(adw_header_bar_new());
  adw_header_bar_set_title_widget(header,
      adw_window_title_new("FedoraWin", "GTK4 / libadwaita on Win32"));
  adw_header_bar_set_decoration_layout(header, ":minimize,maximize,close");
  adw_toolbar_view_add_top_bar(toolbar, GTK_WIDGET(header));

  GtkWidget *page = adw_status_page_new();
  adw_status_page_set_icon_name(ADW_STATUS_PAGE(page),
                                "preferences-desktop-theme-symbolic");
  adw_status_page_set_title(ADW_STATUS_PAGE(page), "Real Adwaita");
  adw_status_page_set_description(ADW_STATUS_PAGE(page),
    "A FedoraWin-owned GTK4 window with a real libadwaita headerbar. "
    "Foreign Windows applications retain their own caption controls.");
  adw_toolbar_view_set_content(toolbar, page);
  adw_application_window_set_content(ADW_APPLICATION_WINDOW(window),
                                     GTK_WIDGET(toolbar));
  gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv) {
  AdwApplication *app = adw_application_new(
      "io.github.rayeen13.FedoraWin.AdwaitaProbe",
      G_APPLICATION_DEFAULT_FLAGS);
  g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);
  int result = g_application_run(G_APPLICATION(app), argc, argv);
  g_object_unref(app);
  return result;
}
