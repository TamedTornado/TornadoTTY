def exact_keys($allowed):
  type == "object" and
  ((keys - $allowed) | length) == 0 and
  (($allowed - keys) | length) == 0;

def stable_id:
  type == "string" and
  test("^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$");

def nonempty_string:
  type == "string" and length > 0 and (contains("\u0000") | not);

def nullable_nonempty_string:
  . == null or nonempty_string;

def contiguous_orders:
  . as $items |
  [$items[].order] | sort == [range(0; ($items | length))];

def valid_command:
  exact_keys(["launch_profile_id"]) and
  (.launch_profile_id |
    type == "string" and test("^[a-z][a-z0-9._-]{0,63}$"));

def valid_agent:
  . == null or (
    exact_keys(["adapter", "resume_id"]) and
    (.adapter | type == "string" and test("^[a-z][a-z0-9-]*$")) and
    (.resume_id | nonempty_string)
  );

def valid_pane:
  exact_keys(["agent", "command", "cwd", "id", "layout", "order", "title"]) and
  (.id | stable_id) and
  (.order | type == "number" and . >= 0 and floor == .) and
  (.title | nullable_nonempty_string) and
  (.layout |
    exact_keys(["column", "row", "row_weight"]) and
    (.column | type == "number" and . >= 0 and floor == .) and
    (.row | type == "number" and . >= 0 and floor == .) and
    (.row_weight | type == "number" and . > 0)
  ) and
  (.cwd | nonempty_string and startswith("/")) and
  (.command | valid_command) and
  (.agent | valid_agent);

def valid_worklane_layout:
  exact_keys(["columns"]) and
  (.columns |
    type == "array" and length > 0 and
    (. as $columns |
      [$columns[].index] | sort == [range(0; ($columns | length))]) and
    all(.[];
      exact_keys(["index", "weight"]) and
      (.index | type == "number" and . >= 0 and floor == .) and
      (.weight | type == "number" and . > 0)
    )
  );

def valid_worklane:
  exact_keys(["active_pane_id", "id", "layout", "order", "panes", "title"]) and
  (.id | stable_id) and
  (.order | type == "number" and . >= 0 and floor == .) and
  (.title | nullable_nonempty_string) and
  (.layout | valid_worklane_layout) and
  (.active_pane_id | stable_id) and
  (.panes |
    type == "array" and length > 0 and
    contiguous_orders and all(.[]; valid_pane)
  ) and
  (. as $worklane |
    $worklane.layout.columns as $columns |
    (all($worklane.panes[]; .layout.column as $column |
      any($columns[]; .index == $column))) and
    (all($columns[]; .index as $column |
      [$worklane.panes[] | select(.layout.column == $column)] as $column_panes |
      ($column_panes | length) > 0 and
      ([$column_panes[].layout.row] | sort) ==
        [range(0; ($column_panes | length))]))) and
  (.active_pane_id as $active_pane |
    any(.panes[]; .id == $active_pane));

def valid_window:
  exact_keys(["active_worklane_id", "id", "order", "worklanes"]) and
  (.id | stable_id) and
  (.order | type == "number" and . >= 0 and floor == .) and
  (.active_worklane_id | stable_id) and
  (.worklanes |
    type == "array" and length > 0 and
    contiguous_orders and all(.[]; valid_worklane)
  ) and
  (.active_worklane_id as $active_worklane |
    any(.worklanes[]; .id == $active_worklane));

def all_ids_are_unique:
  ([.windows[].id] +
   [.windows[].worklanes[].id] +
   [.windows[].worklanes[].panes[].id]) as $ids |
  ($ids | length) == ($ids | unique | length);

def valid_v1:
  exact_keys(["active_window_id", "revision", "schema_version", "windows", "workspace_id"]) and
  .schema_version == 1 and
  (.workspace_id | stable_id) and
  (.revision | type == "number" and . >= 0 and floor == .) and
  (.active_window_id | stable_id) and
  (.windows |
    type == "array" and length > 0 and
    contiguous_orders and all(.[]; valid_window)
  ) and
  (.active_window_id as $active_window |
    any(.windows[]; .id == $active_window)) and
  all_ids_are_unique;

def valid_v0_pane:
  exact_keys(["command_profile", "cwd", "id"]) and
  (.id | stable_id) and
  (.cwd | nonempty_string and startswith("/")) and
  (.command_profile |
    type == "string" and test("^[a-z][a-z0-9._-]{0,63}$"));

def valid_v0_worklane:
  exact_keys(["active_pane", "id", "panes"]) and
  (.id | stable_id) and
  (.active_pane | stable_id) and
  (.panes | type == "array" and length > 0 and all(.[]; valid_v0_pane)) and
  (.active_pane as $active_pane |
    any(.panes[]; .id == $active_pane));

def valid_v0_window:
  exact_keys(["active_worklane", "id", "worklanes"]) and
  (.id | stable_id) and
  (.active_worklane | stable_id) and
  (.worklanes | type == "array" and length > 0 and all(.[]; valid_v0_worklane)) and
  (.active_worklane as $active_worklane |
    any(.worklanes[]; .id == $active_worklane));

def valid_v0:
  exact_keys(["active_window", "schema_version", "windows", "workspace_id"]) and
  .schema_version == 0 and
  (.workspace_id | stable_id) and
  (.active_window | stable_id) and
  (.windows | type == "array" and length > 0 and all(.[]; valid_v0_window)) and
  (.active_window as $active_window |
    any(.windows[]; .id == $active_window));

def migrate_v0:
  . as $root |
  {
    schema_version: 1,
    workspace_id: $root.workspace_id,
    revision: 0,
    active_window_id: $root.active_window,
    windows: [
      $root.windows | to_entries[] |
      . as $window_entry |
      {
        id: $window_entry.value.id,
        order: $window_entry.key,
        active_worklane_id: $window_entry.value.active_worklane,
        worklanes: [
          $window_entry.value.worklanes | to_entries[] |
          . as $worklane_entry |
          {
            id: $worklane_entry.value.id,
            order: $worklane_entry.key,
            title: null,
            layout: {
              columns: [
                {index: 0, weight: 1}
              ]
            },
            active_pane_id: $worklane_entry.value.active_pane,
            panes: [
              $worklane_entry.value.panes | to_entries[] |
              . as $pane_entry |
              {
                id: $pane_entry.value.id,
                order: $pane_entry.key,
                title: null,
                layout: {
                  column: 0,
                  row: $pane_entry.key,
                  row_weight: 1
                },
                cwd: $pane_entry.value.cwd,
                command: {
                  launch_profile_id: $pane_entry.value.command_profile
                },
                agent: null
              }
            ]
          }
        ]
      }
    ]
  };

if env.ZENTTY_SCHEMA_VALIDATION_MODE == "v0-migration" then
  valid_v0 and (migrate_v0 | valid_v1)
else
  valid_v1
end
