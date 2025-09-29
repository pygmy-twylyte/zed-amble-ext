; ===== Definitions =====

(room_def
  room_id: (room_id) @name) @definition.module

(item_def
  item_id: (item_id) @name) @definition.type

(npc_def
  npc_id: (npc_id) @name) @definition.type

(goal_def
  goal_id: (goal_id) @name) @definition.module

(trigger_def
  name: (entity_name) @name) @definition.function

(spinner_def
  name: (spinner_id) @name) @definition.function


; Optional: docstrings (adjacent leading comments)
(
  (comment)+ @doc
  .
  (room_def
    room_id: (room_id) @name) @definition.module
  (#strip! @doc "^#\\s*")
  (#select-adjacent! @doc @definition.module)
)


; ===== References (via wrappers) =====
; These match anywhere you used the *_ref supertypes.

(_room_ref   (room_id)    @name) @reference.module
(_item_ref   (item_id)    @name) @reference.type
(_npc_ref    (npc_id)     @name) @reference.type
(_flag_ref   (flag_name)  @name) @reference.macro
(_goal_ref   (goal_id)    @name) @reference.module
(_spinner_ref (spinner_id) @name) @reference.function
