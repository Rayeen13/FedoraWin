/* FedoraWin-owned GTK4/libadwaita Preferences candidate for Windows.
 * This is an on-demand native surface. It does not patch Windows, Explorer,
 * servicing components, or user data, and it is never injected into another app. */
#include <adwaita.h>

static void apply_theme(void) {
  const char *theme = g_getenv("FEDORAWIN_ADWAITA_THEME");
  AdwStyleManager *style = adw_style_manager_get_default();

  if (g_strcmp0(theme, "dark") == 0)
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_FORCE_DARK);
  else if (g_strcmp0(theme, "light") == 0)
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_FORCE_LIGHT);
  else
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_DEFAULT);
}

static GtkWidget *status_row(const char *title,
                             const char *subtitle,
                             const char *icon_name) {
  GtkWidget *row = adw_action_row_new();
  adw_preferences_row_set_title(ADW_PREFERENCES_ROW(row), title);
  adw_action_row_set_subtitle(ADW_ACTION_ROW(row), subtitle);

  GtkWidget *icon = gtk_image_new_from_icon_name(icon_name);
  gtk_widget_set_valign(icon, GTK_ALIGN_CENTER);
  gtk_widget_add_css_class(icon, "success");
  adw_action_row_add_suffix(ADW_ACTION_ROW(row), icon);
  return row;
}

static GtkWidget *value_row(const char *title, const char *subtitle) {
  GtkWidget *row = adw_action_row_new();
  adw_preferences_row_set_title(ADW_PREFERENCES_ROW(row), title);
  adw_action_row_set_subtitle(ADW_ACTION_ROW(row), subtitle);
  return row;
}

static void activate(GtkApplication *application, gpointer unused) {
  (void)unused;
  apply_theme();

  GtkWidget *window = adw_application_window_new(application);
  gtk_window_set_title(GTK_WINDOW(window), "FedoraWin Preferences");
  gtk_window_set_default_size(GTK_WINDOW(window), 760, 620);

  AdwToolbarView *toolbar = ADW_TOOLBAR_VIEW(adw_toolbar_view_new());
  AdwHeaderBar *header = ADW_HEADER_BAR(adw_header_bar_new());
  adw_header_bar_set_title_widget(
      header, adw_window_title_new("FedoraWin", "Preferences"));
  adw_header_bar_set_decoration_layout(header, ":minimize,maximize,close");
  adw_toolbar_view_add_top_bar(toolbar, GTK_WIDGET(header));

  GtkWidget *page = adw_preferences_page_new();
  adw_preferences_page_set_title(ADW_PREFERENCES_PAGE(page), "FedoraWin");
  adw_preferences_page_set_icon_name(
      ADW_PREFERENCES_PAGE(page), "preferences-system-symbolic");

  GtkWidget *appearance = adw_preferences_group_new();
  adw_preferences_group_set_title(
      ADW_PREFERENCES_GROUP(appearance), "Appearance");
  adw_preferences_group_set_description(
      ADW_PREFERENCES_GROUP(appearance),
      "GNOME 51 visual behavior for FedoraWin-owned surfaces.");
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(appearance),
      value_row("Interface",
                "Real GTK4 + libadwaita on the Windows Win32 backend"));

  const char *theme = g_getenv("FEDORAWIN_ADWAITA_THEME");
  const char *theme_label = g_strcmp0(theme, "dark") == 0 ? "Dark" :
                            g_strcmp0(theme, "light") == 0 ? "Light" :
                            "Follow system";
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(appearance),
      value_row("Color scheme", theme_label));
  adw_preferences_page_add(
      ADW_PREFERENCES_PAGE(page), ADW_PREFERENCES_GROUP(appearance));

  GtkWidget *integration = adw_preferences_group_new();
  adw_preferences_group_set_title(
      ADW_PREFERENCES_GROUP(integration), "Windows integration");
  adw_preferences_group_set_description(
      ADW_PREFERENCES_GROUP(integration),
      "Windows remains the platform authority underneath FedoraWin.");
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(integration),
      status_row("Native window behavior",
                 "Win32 remains responsible for system commands, Snap and DPI",
                 "emblem-ok-symbolic"));
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(integration),
      status_row("Optional frame integration",
                 "Per-app opt-in, reversible, and excluded from protected targets",
                 "emblem-ok-symbolic"));
  adw_preferences_page_add(
      ADW_PREFERENCES_PAGE(page), ADW_PREFERENCES_GROUP(integration));

  GtkWidget *safety = adw_preferences_group_new();
  adw_preferences_group_set_title(
      ADW_PREFERENCES_GROUP(safety), "Safety contract");
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(safety),
      status_row("Windows servicing",
                 "Windows Update, WinSxS and protected system binaries are never patched",
                 "security-high-symbolic"));
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(safety),
      status_row("User data",
                 "Documents, profiles, credentials and application data are untouched",
                 "security-high-symbolic"));
  adw_preferences_page_add(
      ADW_PREFERENCES_PAGE(page), ADW_PREFERENCES_GROUP(safety));

  adw_toolbar_view_set_content(toolbar, page);
  adw_application_window_set_content(
      ADW_APPLICATION_WINDOW(window), GTK_WIDGET(toolbar));
  gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv) {
  AdwApplication *app = adw_application_new(
      "io.github.rayeen13.FedoraWin.Preferences",
      G_APPLICATION_DEFAULT_FLAGS);
  g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);
  int result = g_application_run(G_APPLICATION(app), argc, argv);
  g_object_unref(app);
  return result;
}
