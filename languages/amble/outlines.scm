((room_def
    (room_id) @name)
@item)

((item_def
    (item_id) @name)
@item)

((npc_def
    (npc_id) @name)
@item)

((goal_def
    (goal_block
        (goal_name_stmt (entity_name) @name)))
@item)

((spinner_def
    (spinner_id) @name)
@item)

((trigger_def
    (entity_name) @name)
@item)

((cond_decl
    (cond_name) @name)
@item)

((action_set_decl
    (action_set_name) @name)
@item)
