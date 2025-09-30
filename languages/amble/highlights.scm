; Highlights file for Amble DSL

; Basic Types
(number) @number
(boolean) @boolean

; Entity identifiers
(room_id) @label
(item_id) @type
(npc_id) @title
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
(ovl_item_presence ["present" "absent"] @function)
(ovl_npc_presence ["present" "absent"] @function)
(ovl_flag_status ["set" "unset" "complete"] @function)
(ovl_npc_state_set "here" @keyword)
(ovl_item_posession ["player" "has" "missing"] @function)
(ovl_npc_state ["in" "state"] @function)
(npc_state_set_custom "custom" @constant)

; Trigger specific
(only_once_kw) @keyword
; when clauses
(when_cond (always_event) @property)
(enter_room) @property
(take_item ["take" "item"] @property)
(talk_to_npc ["talk" "to" "npc"] @property)
(open_item ["open" "item"] @property)
(leave_room ["leave" "room"] @property)
(look_at_item ["look" "at" "item"] @property)
(use_item ["use" "item" "ability"] @property)
(give_to_npc ["give" "item" "to" "npc"] @property)
(use_item_on_item ["use" "item" "on" "item" "interaction"] @property)
(act_on_item ["act" "on" "item"] @property)
(take_from_npc ["take" "item" "from" "npc"] @property)
(insert_item_into ["insert" "item" "into"] @property)
(drop_item ["drop" "item"] @property)
(unlock_item ["unlock" "item"] @property)
(ingest_item ["eat" "drink" "inhale"] @property)


; block (if) statements
(cond_any_group ["any" "(" ")"] @constructor)
(cond_all_group ["all" "(" ")"] @constructor)
(cond_has_flag  ["has" "flag"] @property)
(cond_missing_flag ["missing" "flag"] @property)
(cond_has_item  ["has" "item"] @property)
(cond_missing_item ["missing" "item"] @property)
(cond_visited_room ["has" "visited" "room"] @property)
(cond_flag_in_progress ["flag" "in" "progress"] @property)
(cond_flag_complete ["flag" "complete"] @property)
(cond_with_npc ["with" "npc"] @property)
(cond_npc_has_item ["npc" "has" "item"] @property)
(cond_npc_in_state ["npc" "in" "state"] @property)
(cond_player_in_room ["player" "in" "room"] @property)
(cond_container_has_item ["container" "has" "item"] @property)
(cond_chance "chance" @property "%" @number)
(cond_ambient ["ambient" "in" "rooms"] @property)


; block (do) statements
(action_show "show" @function)
(action_add_wedge ["add" "wedge" "width" "spinner"] @function)
(action_add_seq ["add" "seq" "flag"] @function)
(action_replace_item ["replace" "item" "with"] @function)
(action_replace_drop_item ["replace" "drop" "item" "with"] @function)
(action_add_flag ["add" "flag"] @function)
(action_reset_flag ["reset" "flag"] @function)
(action_remove_flag ["remove" "flag"] @function)
(action_advance_flag ["advance" "flag"] @function)
(spawn_action_stem ["spawn" "item"] @function)
(action_spawn_room ["into" "room"] @function)
(action_spawn_container ["into" "in" "container"] @function)
(action_spawn_inventory ["in" "inventory"] @function)
(action_spawn_current_room ["in" "current" "room"] @function)
(action_despawn_item ["despawn" "item"] @function)
(action_award_points ["award" "points"] @function)
(action_lock_item ["lock" "item"] @function)
(action_unlock_item ["unlock" "item"] @function)
(action_lock_exit ["lock" "exit" "from" "direction"] @function)
(action_unlock_exit ["unlock" "exit" "from" "direction"] @function)
(action_reveal_exit ["reveal" "exit" "from" "to" "direction"] @functton)
(action_push_player ["push" "player" "to"] @function)
(action_set_item_desc ["set" "item" "description"] @function)
(action_npc_random_dialogue ["npc" "random" "dialogue"] @function)
(action_npc_says ["npc" "says"] @function)
(action_npc_refuse_item ["npc" "refuse" "item"] @function)
(action_set_npc_active ["set" "npc" "active"] @function)
(action_set_npc_state ["set" "npc" "state"] @function)
(action_deny_read ["deny" "read"] @function)
(action_restrict_item ["restrict" "item"] @function)
(action_give_to_player ["give" "item" "to" "player" "from" "npc"] @function)
(action_set_barred_msg ["set" "barred" "message" "from" "to"] @function)
(action_set_container_state ["set" "container" "state"] @function)
(action_spinner_msg ["spinner" "message"] @function)
(action_schedule_in_or_on ["schedule" "in" "on"] @function)
(action_schedule_in_if ["schedule" "in" "on"] @function)
