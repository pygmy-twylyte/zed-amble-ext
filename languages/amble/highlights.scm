; Highlights file for Amble DSL
(comment) @comment
(number) @number

; ROOMS Highlights
; Basic / Required Elements
(room_def "room" @keyword)
(room_def id: (room_id) @label)
(room_name "name" @keyword name: (room_name) @string)
(room_desc ["desc" "description"] @keyword desc: (room_desc) @string)
(room_visited "visited" @keyword visited: (room_visited) @boolean)

; Exits +/= conditional blocks
(room_exit "exit" @keyword dir: (exit_dir) @variant "->" @punctuation.special dest: (exit_dest) @label)
(exit_stmt ["hidden" "locked"] @keyword)
(required_items_stmt "required_items" @function item_id: (identifier) @label)
(required_flags_stmt "required_flags" @function flag_id: (identifier) @label)
(barred_stmt "barred" @keyword msg: (barred_msg) @string)

; Overlays -- singular and set builders
(overlay_stmt "overlay" @keyword "if" @keyword)
(ovl_text_stmt "text" @keyword text: (ovl_text) @string)
(ovl_flag_status ["flag" "set" "unset" "complete"] @function flag_name: (identifier) @label)
(ovl_item_presence ["item" "present" "absent"] @function item_id: (identifier) @label)
(ovl_item_posession ["player" "has" "missing" "item"] @function item_id: (identifier) @label)
(ovl_npc_presence ["npc" "present" "absent"] @function npc_id: (identifier) @label)
(ovl_npc_state ["npc" "in" "state"] @function npc_id: (identifier) @label )
(npc_state_builtin) @constant
(npc_state_custom "custom" @constant custom_state: (string) @string.special)
(ovl_item_in_room ["item" "in" "room"] @function item_id: (identifier) @label room_id: (identifier) @label)
(ovl_flag_binary ["overlay" "if" "flag"] @function flag_name: (identifier) @label)
(flag_binary_block "set" @variant set_text: (string) @string)
(flag_binary_block "unset" @variant unset_text: (string) @string)
(ovl_presence_pair ["overlay" "if" "item" "npc"] @function (identifier) @label)
(presence_pair_block ["present" "absent"] @variant (string) @string)
(ovl_npc_state_set ["overlay" "if" "npc"] @function npc_id: (identifier) @label "here" @function)
(npc_state_set_line text: (string) @string)
(npc_state_set_custom "custom" @constant state: (identifier) @string.special)


; TRIGGERS Highlights
(trigger_def ["trigger" "when"] @keyword )
(trigger_def name: (string) @string)
(trigger_def once: (only_once_kw) @property)

; when conditions (triggering events)
(when_cond) @keyword
(always_event) @type
(enter_room ["enter" "room"] @type room_id: (_) @label)
(take_item ["take" "item"] @type item_id: (_) @label)
(drop_item ["drop" "item"] @type item_id: (_) @label)
(unlock_item ["unlock" "item"] @type item_id: (_) @label)
(talk_to_npc ["talk" "to" "npc"] @type npc_id: (_) @label)
(open_item ["open" "item"] @type item_id: (_) @label)
(leave_room ["leave" "room"] @type room_id: (_) @label)
(look_at_item ["look" "at" "item"] @type item_id: (_) @label)
(use_item ["use" "item" "ability"] @type
    item_id: (_) @label
    ability: (_) @variable.special
)
(give_to_npc ["give" "item" "to" "npc"] @type
    item_id: (_) @label
    npc_id: (_) @label
)
(use_item_on_item
    ["use" "item" "on" "item" "interaction"] @type
    tool_id: (_) @label
    target_id: (_) @label
    interaction: (_) @variable.special
)
(act_on_item ["act" "on" "item"] @type
    action: (_) @variable.special
    item_id: (_) @label
)
(take_from_npc ["take" "item" "from" "npc"] @type
    item_id: (_) @label
    npc_id: (_) @label
)
(insert_item_into ["insert" "item" "into"] @type
    item_id: (_) @label
)

; trigger IF conditions
(cond_block "if" @keyword)
(cond_any_group "any" @function)
(cond_all_group "all" @function)
(cond_has_flag ["has" "flag"] @function.builtin flag_name: (_) @label)
(cond_missing_flag ["missing" "flag"] @function.builtin flag_name: (_) @label)
(cond_has_item ["has" "item"] @function.builtin item_id: (_) @label)
(cond_missing_item ["missing" "item"] @function.builtin item_id: (_) @label)
(cond_visited_room ["has" "visited" "room"] @function.builtin room_id: (_) @label)
(cond_flag_in_progress ["flag" "in" "progress"] @function.builtin flag_name: (_) @label)
(cond_flag_complete ["flag" "complete"] @function.builtin flag_name: (_) @label)
(cond_with_npc ["with" "npc"] @function.builtin npc_id: (_) @label)
(cond_npc_has_item ["npc" "has" "item"] @function.builtin npc_id: (_) @label item_id: (_) @label)
(cond_npc_in_state ["npc" "in" "state"] @function.builtin npc_id: (_) @label state: (_) @variable.special)
(cond_player_in_room ["player" "in" "room"] @function.builtin room_id: (_) @label)
(cond_container_has_item ["container" "has" "item"] @function.builtin item_id:(_) @label)
(cond_chance ["chance" "%"] @function.builtin pct: (_) @number)
(cond_ambient ["ambient"] @function.builtin spinner: (_) @variant (identifier) @label)
(cond_in_rooms ["in" "rooms"] @function.builtin (identifier) @label )

; trigger "DO" actions
(do_action "do" @keyword)
(action_show "show" @function text: (_) @string)
(action_add_wedge ["add" "wedge" "width" "spinner"] @function
    text: (string) @string
    spinner: (identifier) @variable.special)
(action_add_seq ["add" "seq" "flag" "limit"] @function
    flag: (_) @label)
(action_replace_item ["replace" "item" "with"] @function
    item_id: (_) @label)
(action_replace_drop_item ["replace" "drop" "item" "with"] @function
    item_id: (_) @label)
(action_add_flag ["add" "flag"] @function
    flag: (_) @label)
(action_reset_flag ["reset" "flag"] @function
    flag: (_) @label)
(action_advance_flag ["advance" "flag"] @function
    flag: (_) @label)
(action_remove_flag ["remove" "flag"] @function
    flag: (_) @label)
(spawn_action_stem ["spawn" "item"] @function
    item_id: (_) @label)
(action_spawn_room ["into" "room"] @function
    room: (_) @label)
(action_spawn_container ["into" "in" "container"] @function
    container_id: (_) @label)
(action_spawn_inventory ["in" "inventory"] @function )
(action_spawn_current_room ["in" "current" "room"] @function)
(action_despawn_item ["despawn" "item"] @function
    item_id: (_) @label)
(action_award_points ["award" "points"] @function)
(action_lock_item ["lock" "item"] @function
    item_id: (_) @label)
(action_unlock_item ["unlock" "item"] @function
    item_id: (_) @label)
(action_lock_exit ["lock" "exit" "from" "direction"] @function
    room_id: (_) @label
    direction: (_) @variant)
(action_unlock_exit ["unlock" "exit" "from" "direction"] @function
    room_id: (_) @label
    direction: (_) @variant)
(action_reveal_exit ["reveal" "exit" "from" "to" "direction"] @function
    from_room: (_) @label
    to_room: (_) @label
    direction: (_) @variant)
(action_push_player ["push" "player" "to"] @function
    room_id: (_) @label)
(action_set_item_desc ["set" "item" "description"] @function
    item_id: (_) @label
    text: (_) @string)
(action_npc_random_dialogue ["npc" "random" "dialogue"] @function
    npc_id: (_) @label)
(action_npc_says ["npc" "says"] @function
    npc_id: (_) @label
    text: (_) @string)
(action_npc_refuse_item ["npc" "refuse" "item"] @function
    npc_id: (_) @label
    reason: (_) @string)
(action_set_npc_state ["set" "npc" "state"] @function
    npc_id: (_) @label
    state: (_) @variable.special)
(action_deny_read ["deny" "read"] @function
    reason: (_) @string)
(action_restrict_item ["restrict" "item"] @function
    item_id: (_) @label)
(action_give_to_player ["give""item""to""player""from""npc"] @function
    item_id: (_) @label
    npc_id: (_) @label)
(action_set_barred_msg ["set""barred""message""from""to"] @function
    room_id: (_) @label
    msg: (_) @string)
(container_state) @variant
(action_set_container_state ["set" "container" "state"] @function
    item_id: (_) @label)
(action_spinner_msg ["spinner" "message"] @function
    spinner: (_) @variable.special)
(retry_type "onFalse" @function
    policy: (_) @variant)
(action_schedule_in_or_on ["schedule""in""on"] @function)
(action_schedule_in_if ["schedule""in""on""if"] @function)
(schedule_note) @comment
