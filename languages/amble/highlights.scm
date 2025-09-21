; Highlights file for Amble DSL
(comment) @comment
(number) @number
(boolean) @boolean


(room_id) @constructor
(goal_id) @emphasis
(exit_dest) @constructor
(exit_dir) @property
(item_id) @label
(npc_id) @type
(flag_name) @enum
(spinner) @variant
(entity_name) @title
(entity_desc) @string
(ovl_text) @string
(spinner_text) @string
(player_message) @string.special
(npc_state) @variable
(custom_state) @variable.special
(item_ability) @constant
(item_interaction) @constant.builtin
(retry_policy) @variant
(schedule_note) @hint
(dev_note) @hint
(item_location) @attribute


; ROOMS Highlights
; Basic / Required Elements
(room_def "room" @keyword)
(room_name "name" @keyword)
(room_desc ["desc" "description"] @keyword)
(room_visited "visited" @keyword visited: (room_visited) @boolean)

; Exits +/= conditional blocks
(room_exit "exit" @keyword "->" @punctuation.special)
(exit_stmt ["hidden" "locked"] @keyword)
(required_items_stmt "required_items" @function)
(required_flags_stmt "required_flags" @function)
(barred_stmt "barred" @keyword)

; Overlays -- singular and set builders
(overlay_stmt ["overlay""if"] @keyword)
(ovl_text_stmt "text" @keyword)
(ovl_flag_status ["flag" "set" "unset" "complete"] @function)
(ovl_item_presence ["item" "present" "absent"] @function)
(ovl_item_posession ["player" "has" "missing" "item"] @function)
(ovl_npc_presence ["npc" "present" "absent"] @function)
(ovl_npc_state ["npc" "in" "state"] @function)
(npc_state_builtin) @constant
(npc_state_custom "custom" @constant)
(ovl_item_in_room ["item" "in" "room"] @function)
(ovl_flag_binary ["overlay" "if" "flag"] @function)
(flag_binary_block "set" @variant)
(flag_binary_block "unset" @variant)
(ovl_presence_pair ["overlay" "if" "item" "npc"] @function)
(presence_pair_block ["present" "absent"] @variant)
(ovl_npc_state_set ["overlay" "if" "npc" "here"] @function)
(npc_state_set_custom "custom" @constant)


; TRIGGERS Highlights
(trigger_def ["trigger" "when"] @keyword )
(trigger_def once: (only_once_kw) @property)

; when conditions (triggering events)
(when_cond) @keyword
(always_event) @type
(enter_room ["enter" "room"] @type)
(take_item ["take" "item"] @type)
(drop_item ["drop" "item"] @type)
(unlock_item ["unlock" "item"] @type)
(talk_to_npc ["talk" "to" "npc"] @type)
(open_item ["open" "item"] @type)
(leave_room ["leave" "room"] @type)
(look_at_item ["look" "at" "item"] @type)
(use_item ["use" "item" "ability"] @type)
(give_to_npc ["give" "item" "to" "npc"] @type)
(use_item_on_item
    ["use" "item" "on" "item" "interaction"] @type)
(act_on_item ["act" "on" "item"] @type)
(take_from_npc ["take" "item" "from" "npc"] @type)
(insert_item_into ["insert" "item" "into"] @type)

; trigger IF conditions
(cond_block "if" @keyword)
(cond_any_group "any" @function)
(cond_all_group "all" @function)
(cond_has_flag ["has" "flag"] @function.builtin)
(cond_missing_flag ["missing" "flag"] @function.builtin )
(cond_has_item ["has" "item"] @function.builtin)
(cond_missing_item ["missing" "item"] @function.builtin)
(cond_visited_room ["has" "visited" "room"] @function.builtin)
(cond_flag_in_progress ["flag" "in" "progress"] @function.builtin)
(cond_flag_complete ["flag" "complete"] @function.builtin)
(cond_with_npc ["with" "npc"] @function.builtin)
(cond_npc_has_item ["npc" "has" "item"] @function.builtin)
(cond_npc_in_state ["npc" "in" "state"] @function.builtin)
(cond_player_in_room ["player" "in" "room"] @function.builtin)
(cond_container_has_item ["container" "has" "item"] @function.builtin)
(cond_chance "chance" @function.builtin "%" @number)
(cond_ambient ["ambient"] @function.builtin)
(cond_in_rooms ["in" "rooms"] @function.builtin)

; trigger "DO" actions
(do_action "do" @keyword)
(action_show "show" @function)
(action_add_wedge ["add" "wedge" "width" "spinner"] @function)
(action_add_seq ["add" "seq" "flag" "limit"] @function)
(action_replace_item ["replace" "item" "with"] @function)
(action_replace_drop_item ["replace" "drop" "item" "with"] @function)
(action_add_flag ["add" "flag"] @function)
(action_reset_flag ["reset" "flag"] @function)
(action_advance_flag ["advance" "flag"] @function)
(action_remove_flag ["remove" "flag"] @function)
(spawn_action_stem ["spawn" "item"] @function)
(action_spawn_room ["into" "room"] @function)
(action_spawn_container ["into" "in" "container"] @function)
(action_spawn_inventory ["in" "inventory"] @function )
(action_spawn_current_room ["in" "current" "room"] @function)
(action_despawn_item ["despawn" "item"] @function)
(action_award_points ["award" "points"] @function)
(action_lock_item ["lock" "item"] @function)
(action_unlock_item ["unlock" "item"] @function)
(action_lock_exit ["lock" "exit" "from" "direction"] @function)
(action_unlock_exit ["unlock" "exit" "from" "direction"] @function)
(action_reveal_exit ["reveal" "exit" "from" "to" "direction"] @function)
(action_push_player ["push" "player" "to"] @function)
(action_set_item_desc ["set" "item" "description"] @function)
(action_npc_random_dialogue ["npc" "random" "dialogue"] @function)
(action_npc_says ["npc" "says"] @function)
(action_npc_refuse_item ["npc" "refuse" "item"] @function)
(action_set_npc_state ["set" "npc" "state"] @function)
(action_deny_read ["deny" "read"] @function)
(action_restrict_item ["restrict" "item"] @function)
(action_give_to_player ["give""item""to""player""from""npc"] @function)
(action_set_barred_msg ["set""barred""message""from""to"] @function)
(container_state) @variant
(action_set_container_state ["set" "container" "state"] @function)
(action_spinner_msg ["spinner" "message"] @function)
(retry_type "onFalse" @function)
(action_schedule_in_or_on ["schedule""in""on"] @function)
(action_schedule_in_if ["schedule""in""on""if"] @function)

; ITEMS Highlights
(item_def "item" @keyword)
(item_name_stmt "name" @keyword)
(item_desc_stmt ["desc" "description"] @keyword)
(item_portable_stmt "portable" @keyword)
(item_loc_stmt "location" @keyword)
(item_text_stmt "text" @keyword)
(item_ability_stmt "ability" @keyword)
(item_requires_stmt ["requires" "to"] @keyword)
(item_container_stmt ["container" "state"] @keyword)

; NPCS Highlights
(npc_def "npc" @keyword)
(npc_name_stmt "name" @keyword)
(npc_desc_stmt ["desc" "description"] @keyword)
(npc_loc_stmt "location" @keyword (npc_location ["room" "nowhere"] @variant))
(npc_state_stmt "state" @keyword)
(npc_dialogue_block "dialogue" @keyword (npc_dialogue) @string)
(npc_movement_stmt "movement" @keyword)
(timing_stmt "timing" @keyword)
(active_stmt "active" @keyword)

; SPINNERS Hightlights
(spinner_def "spinner" @keyword)
(spinner_stmt ["wedge" "width"] @property)

; GOALS Highlights
(goal_def "goal" @keyword)
(goal_name_stmt "name" @property)
(goal_desc_stmt "desc" @property)
(goal_group_stmt "group" @property)
(goal_start_stmt ["start" "when"] @property)
(goal_done_stmt ["done" "when"] @property)
(gc_has_item ["has""item"] @function)
(gc_has_flag ["has" "flag"] @function)
(gc_flag_progress ["flag" "in" "progress"] @function)
(gc_goal_complete ["goal" "complete"] @function)
(gc_flag_complete ["flag" "complete"] @function)
(gc_reached_room ["reached" "room"] @function)
(gc_missing_flag ["flag" "missing"] @function)
