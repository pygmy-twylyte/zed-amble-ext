; Curly brace blocks
[
  "{"
  "}"
] @bracket

; Parentheses
[
  "("
  ")"
] @bracket

; Block structures with explicit open/close
(room_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(item_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(npc_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(trigger_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(goal_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(spinner_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(ovl_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(exit_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(flag_binary_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(presence_pair_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(npc_state_set_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

(cond_block
  "{" @bracket.open
  "}" @bracket.close) @bracket.container

; Parenthetical structures
(set_list
  "(" @bracket.open
  ")" @bracket.close) @bracket.container

(cond_any_group
  "(" @bracket.open
  ")" @bracket.close) @bracket.container

(cond_all_group
  "(" @bracket.open
  ")" @bracket.close) @bracket.container

(required_items_stmt
  "(" @bracket.open
  ")" @bracket.close) @bracket.container

(required_flags_stmt
  "(" @bracket.open
  ")" @bracket.close) @bracket.container

(npc_state_set_custom
  "(" @bracket.open
  ")" @bracket.close) @bracket.container
