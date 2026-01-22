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

(set_name) @variable

; Things with defined variants
(item_ability) @variant

(ability_name) @variant

(item_interaction) @variant

(custom_state) @variant

(npc_state_builtin) @constant

; String-type nodes
(entity_name) @string.special

(entity_desc) @string

(ovl_text) @string

(exit_dir) @string.special

(player_message) @string

(item_detail_text) @string

(wedge_text) @string

(quote) @string

(npc_dialogue) @string

(spinner_text) @string

(string) @string

; Comments / notes
(comment) @comment

(dev_note) @emphasis

(schedule_note) @comment.doc

; Global Markups
[
  "if"
  "do"
  "when"
] @keyword

[
  "("
  ")"
  "{"
  "}"
] @punctuation.bracket

"," @punctuation.delimiter

[
  "true"
  "false"
] @boolean

(set_decl
  [
    "let"
    "set"
  ] @keyword
  "=" @operator)

(game_def
  [
    "game"
    "title"
  ] @keyword)

(game_title
  "title" @keyword)

(game_slug
  "slug" @keyword
  (string) @string)

(game_author
  "author" @keyword)

(game_version
  "version" @keyword)

(game_blurb
  "blurb" @keyword
  (string) @string.special)

(game_intro
  "intro" @keyword)

(game_player
  "player" @keyword)

(player_name
  "name" @property)

(player_desc
  [
    "desc"
    "description"
  ] @property)

(player_max_hp
  "max_hp" @property
  max_hp: (pos_int) @number)

(player_start
  [
    "start"
    "room"
  ] @property
  room_id: (_room_ref) @room_id)

(game_scoring
  "scoring" @keyword)

(scoring_title
  "report_title" @property
  title: (entity_name) @string.special)

(scoring_rank
  "rank" @property
  (score_threshold) @number
  (entity_name) @string.special
  (entity_desc) @string)

; Room specific
(room_def
  "room" @keyword)

(room_name
  "name" @keyword)

(room_visited
  "visited" @keyword)

(room_desc
  [
    "desc"
    "description"
  ] @keyword)

(ovl_flag_binary
  "overlay" @keyword
  [
    "if"
    "flag"
  ] @function)

(ovl_presence_pair
  "overlay" @keyword
  [
    "if"
    "item"
  ] @function)

(overlay_stmt
  "overlay" @keyword)

(presence_pair_block
  [
    "present"
    "absent"
  ] @property)

(flag_binary_block
  [
    "set"
    "unset"
  ] @property)

(room_exit
  "exit" @keyword
  "->" @punctuation.special)

(required_flags_stmt
  "required_flags" @attribute)

(required_items_stmt
  "required_items" @attribute)

(barred_stmt
  "barred" @attribute)

(ovl_text_stmt
  "text" @property)

(ovl_item_presence
  [
    "item"
    "present"
    "absent"
  ] @function)

(ovl_npc_presence
  [
    "npc"
    "present"
    "absent"
  ] @function)

(ovl_flag_status
  [
    "flag"
    "set"
    "unset"
    "complete"
  ] @function)

(ovl_npc_state_set
  "overlay" @keyword
  [
    "if"
    "npc"
    "here"
  ] @function)

(ovl_item_posession
  [
    "player"
    "has"
    "missing"
    "item"
  ] @function)

(ovl_npc_state
  [
    "npc"
    "in"
    "state"
  ] @function)

(npc_state_set_custom
  "custom" @constant)

; Trigger Highlights
(trigger_def
  "trigger" @keyword)

(only_once_kw) @keyword

(_when_event) @property

(_trigger_cond_atom) @constructor

(_action_type) @function

(off_or_state
  "off" @comment)

(priority_clause
  "priority" @embedded)

; Item Highlights
(item_def
  "item" @keyword)

(_item_stmt) @keyword

(container_state) @variable.special

(movability) @variant

(item_location
  [
    "room"
    "inventory"
    "player"
    "npc"
    "nowhere"
    "chest"
  ] @variant.builtin)

(consumable_uses
  "uses_left" @property.builtin)

(consumable_consume_on
  [
    "consume_on"
    "ability"
  ] @property.builtin)

(consumable_when_consumed
  "when_consumed" @property.builtin
  (when_consumed_opt) @variant)

; NPC Highlights
(npc_def
  "npc" @keyword)

(npc_name_stmt
  "name" @keyword)

(npc_desc_stmt
  [
    "desc"
    "description"
  ] @keyword)

(npc_loc_stmt
  "location" @keyword
  (npc_location
    [
      "room"
      "nowhere"
    ] @variant.builtin))

(npc_max_hp_stmt
  "max_hp" @keyword
  (pos_int) @number)

(npc_state_stmt
  "state" @keyword)

(npc_dialogue_block
  "dialogue" @keyword)

(npc_movement_stmt
  [
    "movement"
    "rooms"
  ] @keyword)

(movement_type) @variable.builtin

(timing_stmt
  "timing" @keyword
  (timing) @variable.special)

(active_stmt
  "active" @keyword)

; Spinner Highlights
(spinner_def
  "spinner" @keyword)

(spinner_stmt
  [
    "wedge"
    "width"
  ] @keyword)

; Goal Highlights
(goal_def
  "goal" @keyword)

(goal_name_stmt
  "name" @keyword)

(goal_desc_stmt
  [
    "desc"
    "description"
  ] @keyword)

(goal_group_stmt
  "group" @keyword)

(goal_group) @variant.builtin

(goal_start_stmt
  [
    "start"
    "when"
  ] @keyword)

(goal_done_stmt
  [
    "done"
    "when"
  ] @keyword)

(goal_fail_stmt
  [
    "fail"
    "when"
  ] @keyword)

(_goal_cond) @property
