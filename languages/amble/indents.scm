; Increase indentation for block contents
[
  (room_block)
  (item_block)
  (npc_block)
  (trigger_block)
  (goal_block)
  (spinner_block)
  (ovl_block)
  (exit_block)
  (flag_binary_block)
  (presence_pair_block)
  (npc_state_set_block)
  (cond_block)
] @indent

; Increase indentation for parenthetical lists
[
  (set_list)
  (cond_any_group)
  (cond_all_group)
  (required_items_stmt)
  (required_flags_stmt)
] @indent

; Opening braces increase indentation
"{" @indent

; Closing braces decrease indentation
"}" @end

; Opening parentheses increase indentation
"(" @indent

; Closing parentheses decrease indentation
")" @end
