; Top-level definitions
(room_def) @fold

(item_def) @fold

(npc_def) @fold

(trigger_def) @fold

(goal_def) @fold

(spinner_def) @fold

(set_decl) @fold

; Block structures
(room_block
  "{" @fold.start
  "}" @fold.end) @fold

(item_block
  "{" @fold.start
  "}" @fold.end) @fold

(npc_block
  "{" @fold.start
  "}" @fold.end) @fold

(trigger_block
  "{" @fold.start
  "}" @fold.end) @fold

(goal_block
  "{" @fold.start
  "}" @fold.end) @fold

(spinner_block
  "{" @fold.start
  "}" @fold.end) @fold

(ovl_block
  "{" @fold.start
  "}" @fold.end) @fold

(exit_block
  "{" @fold.start
  "}" @fold.end) @fold

(flag_binary_block
  "{" @fold.start
  "}" @fold.end) @fold

(presence_pair_block
  "{" @fold.start
  "}" @fold.end) @fold

(npc_state_set_block
  "{" @fold.start
  "}" @fold.end) @fold

(cond_block
  "{" @fold.start
  "}" @fold.end) @fold

; Parenthetical structures
(set_list
  "(" @fold.start
  ")" @fold.end) @fold

(cond_any_group
  "(" @fold.start
  ")" @fold.end) @fold

(cond_all_group
  "(" @fold.start
  ")" @fold.end) @fold

(required_items_stmt
  "(" @fold.start
  ")" @fold.end) @fold

(required_flags_stmt
  "(" @fold.start
  ")" @fold.end) @fold

(npc_state_set_custom
  "(" @fold.start
  ")" @fold.end) @fold

; Multi-line strings (triple-quoted)
(string) @fold

; Comments
(comment) @fold

; Complex overlay statements
(overlay_stmt) @fold

(ovl_flag_binary) @fold

(ovl_presence_pair) @fold

(ovl_npc_state_set) @fold

; Action sequences in triggers
(action_schedule_in_or_on) @fold

(action_schedule_in_if) @fold

; Complex when conditions with multiple parts
(use_item_on_item) @fold

(give_to_npc) @fold

(take_from_npc) @fold

(insert_item_into) @fold
