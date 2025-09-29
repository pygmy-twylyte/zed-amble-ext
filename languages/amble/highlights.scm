; Highlights file for Amble DSL

; Basic Types
(number) @number
(boolean) @boolean

; Entity identifiers
(room_id) @label
(item_id) @type
(npc_id) @tag
(flag_name) @emphasis
(spinner_id) @variable
(goal_id) @enum

; Things with defined variants
(item_ability) @variant
(item_interaction) @variant
(custom_state) @variant
(npc_state_builtin) @constant

; String-type nodes
(entity_name) @string.special
(entity_desc) @string
(ovl_text) @string
(exit_dir) @string.special
(player_message) @string

; Comments / notes
(comment) @comment
(dev_note) @comment.doc
(schedule_note) @comment.doc

; Global Markups
[ "room" "item" "npc" "goal" "spinner" "trigger" "flag"
"if" "do" "when" "name" "desc" "description" ] @keyword

[ "(" ")" "{" "}" ] @punctuation.bracket
[ "," ] @punctuation.delimiter

[ "true" "false" ] @boolean

; Room specific
(_room_stmt ["visited" "exit" "overlay"] @keyword)
(presence_pair_block ["present" "absent"] @property)
(flag_binary_block ["set" "unset"] @property)
(room_exit "->" @punctuation.special)
(required_flags_stmt "required_flags" @attribute)
(required_items_stmt "required_items" @attribute)
(barred_stmt "barred" @property)
(ovl_text_stmt "text" @property)
(ovl_item_presence ["item" "present" "absent"] @function)
(ovl_npc_presence ["npc" "present" "absent"] @function)
(ovl_flag_status ["flag" "set" "unset" "complete"] @function)
(ovl_npc_state_set "here" @keyword)
(ovl_item_posession ["player" "has" "missing" "item"] @function)
(ovl_npc_state ["npc" "in" "state"] @function)
(npc_state_set_custom "custom" @constant)
