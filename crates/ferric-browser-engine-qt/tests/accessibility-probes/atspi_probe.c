#include <atspi/atspi.h>
#include <glib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static AtspiAccessible *find_named(AtspiAccessible *node, const char *wanted, int depth)
{
    if (!node || depth > 24)
        return NULL;
    GError *error = NULL;
    gchar *name = atspi_accessible_get_name(node, &error);
    if (error) {
        g_clear_error(&error);
    } else if (name && strcmp(name, wanted) == 0) {
        g_free(name);
        return g_object_ref(node);
    }
    g_free(name);

    gint count = atspi_accessible_get_child_count(node, &error);
    if (error) {
        g_clear_error(&error);
        return NULL;
    }
    for (gint i = 0; i < count; ++i) {
        AtspiAccessible *child = atspi_accessible_get_child_at_index(node, i, &error);
        if (error) {
            g_clear_error(&error);
            continue;
        }
        AtspiAccessible *found = find_named(child, wanted, depth + 1);
        g_object_unref(child);
        if (found)
            return found;
    }
    return NULL;
}

static void dump_tree(AtspiAccessible *node, int depth)
{
    if (!node || depth > 16)
        return;
    GError *error = NULL;
    gchar *name = atspi_accessible_get_name(node, &error);
    if (error)
        g_clear_error(&error);
    gchar *role = atspi_accessible_get_role_name(node, &error);
    if (error)
        g_clear_error(&error);
    printf("node depth=%d role=%s name=%s", depth, role ? role : "?", name ? name : "");
    AtspiComponent *component = atspi_accessible_get_component_iface(node);
    if (component) {
        AtspiRect *rect = atspi_component_get_extents(component, ATSPI_COORD_TYPE_SCREEN, &error);
        if (error) {
            printf(" bounds=error");
            g_clear_error(&error);
        } else if (rect) {
            printf(" bounds=%d,%d,%d,%d", rect->x, rect->y, rect->width, rect->height);
            g_free(rect);
        }
    } else {
        printf(" bounds=unavailable");
    }
    if (atspi_accessible_is_action(node)) {
        AtspiAction *actions = atspi_accessible_get_action_iface(node);
        gint count = atspi_action_get_n_actions(actions, &error);
        if (error)
            g_clear_error(&error);
        for (gint i = 0; i < count; ++i) {
            gchar *action = atspi_action_get_action_name(actions, i, &error);
            if (error)
                g_clear_error(&error);
            printf(" action[%d]=%s", i, action ? action : "?");
            g_free(action);
        }
    }
    putchar('\n');
    fflush(stdout);
    g_free(name);
    g_free(role);

    gint count = atspi_accessible_get_child_count(node, &error);
    if (error) {
        g_clear_error(&error);
        return;
    }
    for (gint i = 0; i < count; ++i) {
        AtspiAccessible *child = atspi_accessible_get_child_at_index(node, i, &error);
        if (error) {
            g_clear_error(&error);
            continue;
        }
        dump_tree(child, depth + 1);
        g_object_unref(child);
    }
}

int main(void)
{
    if (atspi_init() != 0) {
        fputs("atspi_init failed\n", stderr);
        return 2;
    }
    AtspiAccessible *desktop = atspi_get_desktop(0);
    if (!desktop) {
        fputs("AT-SPI desktop unavailable\n", stderr);
        return 3;
    }
    puts("AT-SPI tree snapshot");

    AtspiAccessible *qml = find_named(desktop, "S24 QML safe action", 0);
    AtspiAccessible *web = NULL;
    AtspiAccessible *closed = NULL;
    AtspiAccessible *canvas = NULL;
    for (int attempt = 0; attempt < 5; ++attempt) {
        web = find_named(desktop, "S24 web safe action", 0);
        canvas = find_named(desktop, "S24 labeled canvas", 0);
        closed = find_named(desktop, "S24 closed-root safe action", 0);
        if (web && canvas)
            break;
        sleep(1);
    }
    dump_tree(desktop, 0);
    printf("provider targets qml=%d web=%d closed_root=%d labeled_canvas=%d\n",
           !!qml, !!web, !!closed, !!canvas);

    gboolean qml_action = FALSE;
    if (qml && atspi_accessible_is_action(qml)) {
        AtspiAction *actions = atspi_accessible_get_action_iface(qml);
        GError *error = NULL;
        gint count = atspi_action_get_n_actions(actions, &error);
        if (error)
            g_clear_error(&error);
        for (gint i = 0; i < count; ++i) {
            gchar *action = atspi_action_get_action_name(actions, i, &error);
            if (error)
                g_clear_error(&error);
            if (action && g_ascii_strcasecmp(action, "press") == 0) {
                qml_action = atspi_action_do_action(actions, i, &error);
                g_free(action);
                break;
            }
            g_free(action);
            if (error) {
                g_clear_error(&error);
                qml_action = FALSE;
            }
        }
    }
    printf("qml press accepted=%d\n", qml_action);
    fflush(stdout);

    // The fixture hides this QML target after 9 seconds. Keep its original
    // AT-SPI object reference and check that it becomes defunct afterwards.
    sleep(7);
    gboolean qml_defunct = FALSE;
    gboolean qml_visible = FALSE;
    if (qml) {
        AtspiStateSet *states = atspi_accessible_get_state_set(qml);
        qml_defunct = states && atspi_state_set_contains(states, ATSPI_STATE_DEFUNCT);
        qml_visible = states && atspi_state_set_contains(states, ATSPI_STATE_VISIBLE);
        if (states)
            g_object_unref(states);
    }
    AtspiAccessible *qml_after = find_named(desktop, "S24 QML safe action", 0);
    gboolean qml_still_enumerated = qml_after != NULL;
    gboolean stale_action = FALSE;
    if (qml && atspi_accessible_is_action(qml)) {
        AtspiAction *actions = atspi_accessible_get_action_iface(qml);
        GError *error = NULL;
        gint count = atspi_action_get_n_actions(actions, &error);
        if (error)
            g_clear_error(&error);
        for (gint i = 0; i < count; ++i) {
            gchar *action = atspi_action_get_action_name(actions, i, &error);
            if (error)
                g_clear_error(&error);
            if (action && g_ascii_strcasecmp(action, "press") == 0) {
                stale_action = atspi_action_do_action(actions, i, &error);
                g_free(action);
                break;
            }
            g_free(action);
            if (error) {
                g_clear_error(&error);
                stale_action = FALSE;
            }
        }
    }
    printf("qml stale target defunct=%d visible=%d still_enumerated=%d stale_press_accepted=%d\n",
           qml_defunct, qml_visible, qml_still_enumerated, stale_action);
    printf("qml provider result action_once=%d exact_removal=%d\n",
           qml_action, qml_defunct && !qml_still_enumerated && !stale_action);
    fflush(stdout);

    int result = qml && qml_action && !qml_still_enumerated && !stale_action ? 0 : 1;
    printf("atspi helper result=%d qml=%d qml_action=%d qml_still_enumerated=%d stale_action=%d\n",
           result, !!qml, qml_action, qml_still_enumerated, stale_action);
    fflush(stdout);
    g_clear_object(&qml_after);
    g_clear_object(&qml);
    g_clear_object(&web);
    g_clear_object(&closed);
    g_clear_object(&canvas);
    g_object_unref(desktop);
    atspi_exit();
    return result;
}
