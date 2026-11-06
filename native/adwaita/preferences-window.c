/* FedoraWin-owned GTK4/libadwaita Preferences candidate for Windows.
 * This is an on-demand native surface. It does not patch Windows, Explorer,
 * servicing components, or user data, and it is never injected into another app. */
#include <adwaita.h>
#include <stdio.h>

typedef struct {
  AdwComboRow *theme_row;
  AdwComboRow *accent_row;
} PreferencesState;

static const char *theme_values[] = {"system", "light", "dark"};
static const char *accent_values[] = {
    "blue", "teal", "green", "yellow", "orange",
    "red", "pink", "purple", "slate"};

static void apply_theme_name(const char *theme) {
  AdwStyleManager *style = adw_style_manager_get_default();

  if (g_strcmp0(theme, "dark") == 0)
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_FORCE_DARK);
  else if (g_strcmp0(theme, "light") == 0)
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_FORCE_LIGHT);
  else
    adw_style_manager_set_color_scheme(style, ADW_COLOR_SCHEME_DEFAULT);
}

static guint selected_theme(const char *theme) {
  if (g_strcmp0(theme, "light") == 0)
    return 1;
  if (g_strcmp0(theme, "dark") == 0)
    return 2;
  return 0;
}

static guint selected_accent(const char *accent) {
  for (guint index = 0; index < G_N_ELEMENTS(accent_values); index++) {
    if (g_strcmp0(accent, accent_values[index]) == 0)
      return index;
  }
  return 0;
}

static void emit_appearance(PreferencesState *state) {
  guint theme_index = adw_combo_row_get_selected(state->theme_row);
  guint accent_index = adw_combo_row_get_selected(state->accent_row);
  if (theme_index >= G_N_ELEMENTS(theme_values) ||
      accent_index >= G_N_ELEMENTS(accent_values))
    return;

  const char *theme = theme_values[theme_index];
  const char *accent = accent_values[accent_index];
  apply_theme_name(theme);
  g_print("FEDORAWIN_APPEARANCE\t%s\t%s\n", theme, accent);
  fflush(stdout);
}

static void on_appearance_changed(GObject *object,
                                  GParamSpec *parameter,
                                  gpointer user_data) {
  (void)object;
  (void)parameter;
  emit_appearance((PreferencesState *)user_data);
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

static GtkWidget *combo_row(const char *title,
                            const char *subtitle,
                            const char *const *labels,
                            guint selected) {
  GtkStringList *model = gtk_string_list_new(labels);
  GtkWidget *row = adw_combo_row_new();
  adw_preferences_row_set_title(ADW_PREFERENCES_ROW(row), title);
  adw_action_row_set_subtitle(ADW_ACTION_ROW(row), subtitle);
  adw_combo_row_set_model(ADW_COMBO_ROW(row), G_LIST_MODEL(model));
  adw_combo_row_set_selected(ADW_COMBO_ROW(row), selected);
  g_object_unref(model);
  return row;
}

static void activate(GtkApplication *application, gpointer unused) {
  (void)unused;
  const char *theme = g_getenv("FEDORAWIN_ADWAITA_THEME");
  const char *accent = g_getenv("FEDORAWIN_ADWAITA_ACCENT");
  apply_theme_name(theme);

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

  const char *theme_labels[] = {"Follow system", "Light", "Dark", NULL};
  const char *accent_labels[] = {
      "Blue", "Teal", "Green", "Yellow", "Orange",
      "Red", "Pink", "Purple", "Slate", NULL};
  GtkWidget *theme_row = combo_row(
      "Color scheme", "Applied live to FedoraWin surfaces and eligible native frames",
      theme_labels, selected_theme(theme));
  GtkWidget *accent_row = combo_row(
      "Accent", "Applied live to FedoraWin shell surfaces",
      accent_labels, selected_accent(accent));
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(appearance), theme_row);
  adw_preferences_group_add(
      ADW_PREFERENCES_GROUP(appearance), accent_row);
  adw_preferences_page_add(
      ADW_PREFERENCES_PAGE(page), ADW_PREFERENCES_GROUP(appearance));

  PreferencesState *state = g_new0(PreferencesState, 1);
  state->theme_row = ADW_COMBO_ROW(theme_row);
  state->accent_row = ADW_COMBO_ROW(accent_row);
  g_signal_connect(state->theme_row, "notify::selected",
                   G_CALLBACK(on_appearance_changed), state);
  g_signal_connect(state->accent_row, "notify::selected",
                   G_CALLBACK(on_appearance_changed), state);
  g_object_set_data_full(
      G_OBJECT(window), "fedorawin-preferences-state", state, g_free);

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
