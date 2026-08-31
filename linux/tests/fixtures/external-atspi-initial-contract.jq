def nodes: [.applications[0] | .. | objects | select(has("role"))];

.schema_version == 1 and
(.applications | length) == 1 and
.applications[0].name == "zentty" and
.applications[0].role == "application" and
.applications[0].process_id == $pid and
(nodes | any(.name == "Zentty" and .role == "frame" and
             (.states | index("active")) != null)) and
(nodes | any(.name == "Worklane 1, shell" and .role == "push button" and
             (.states | index("selected")) != null)) and
(nodes | any(.name == "shell" and .role == "push button" and
             (.states | index("selected")) != null)) and
(nodes | any(.name == "Terminal pane pane-1" and .role == "terminal" and
             (.states | index("focused")) != null)) and
(nodes | any(.name == "Toggle sidebar" and .role == "push button")) and
(nodes | any(.name == "Drag pane" and
             .description == "Draggable pane. Drop on a pane edge, column boundary, or worklane.")) and
(nodes | any(.description == "Pane drop destination: worklane worklane-1")) and
(nodes | any(.description == "Pane drop destination: pane pane-1 in worklane worklane-1")) and
(nodes | any(.name == "Add Pane Right" and .role == "push button" and
             (.actions | index("click")) != null)) and
(nodes | any(.name == "New Pane Below" and .role == "push button")) and
(nodes | any(.name == "Close Pane" and .role == "push button")) and
(nodes | map(select(.name == "Pane actions")) | length) == 2 and
(nodes | any((.states | index("focused")) != null))
