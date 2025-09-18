(comment) @comment

(room_def "room" @keyword)
(room_def id: (room_id) @label)
(room_name "name" @keyword name: (room_name) @string)
(room_desc "desc" @keyword desc: (room_desc) @string)
(room_visited "visited" @keyword visited: (room_visited) @boolean)

(room_exit "exit" @keyword dir: (exit_dir) @variant "->" @punctuation.special dest: (exit_dest) @label)
(required_items_stmt "required_items" @function item_id: (identifier) @label)
(required_flags_stmt "required_flags" @function flag_id: (identifier) @label)
(barred_stmt "barred" @keyword msg: (barred_msg) @string)

(overlay_stmt "overlay" @keyword "if" @keyword)
(ovl_text_stmt "text" @keyword text: (ovl_text) @string)
(ovl_flag_set "flag" @function "set" @function flag: (flag_name) @label)
(ovl_flag_unset "flag" @function "unset" @function flag: (flag_name) @label)
