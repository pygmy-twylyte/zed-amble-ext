/**
 * @file DSL for the Amble game engine.
 * @author djvb <djvbmd@protonmail.com>
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: "amble",

  word: ($) => $.identifier,
  extras: ($) => [$.comment, /[\s\r\n\t]/],

  supertypes: ($) => [
    $._room_ref,
    $._item_ref,
    $._npc_ref,
    $._flag_ref,
    $._goal_ref,
    $._spinner_ref,
    $._set_ref,
    $._game_stmt,
    $._player_stmt,
    $._scoring_stmt,
    $._room_stmt,
    $._item_stmt,
    $._npc_stmt,
    $._goal_stmt,
    $._trigger_stmt,
    $._when_event,
    $._trigger_cond_atom,
    $._action_type,
    $._goal_cond,
  ],

  rules: {
    source_file: ($) =>
      repeat1(
        choice(
          $.set_decl,
          $.game_def,
          $.room_def,
          $.item_def,
          $.npc_def,
          $.trigger_def,
          $.spinner_def,
          $.goal_def,
        ),
      ),

    comment: ($) => token(seq("#", /.*/)),

    // create named nodes for common semantic types
    set_name: ($) => alias($.identifier, $.set_name),
    room_id: ($) => alias($.identifier, $.room_id),
    item_id: ($) => alias($.identifier, $.item_id),
    npc_id: ($) => alias($.identifier, $.npc_id),
    spinner_id: ($) => alias($.identifier, $.spinner_id),
    goal_id: ($) => alias($.identifier, $.goal_id),
    flag_name: ($) => alias($.identifier, $.flag_name),
    custom_state: ($) => alias($.identifier, $.custom_state),
    item_ability: ($) => alias($.identifier, $.item_ability),
    item_interaction: ($) => alias($.identifier, $.item_interaction),
    exit_dir: ($) => alias(choice($.identifier, $.string), $.exit_dir),
    entity_name: ($) => alias($.string, $.entity_name),
    entity_desc: ($) => alias($.string, $.entity_desc),

    // reference wrappers... simplify definitions vs. references in tags.scm, highlighting
    _room_ref: ($) => $.room_id,
    _item_ref: ($) => $.item_id,
    _npc_ref: ($) => $.npc_id,
    _flag_ref: ($) => $.flag_name,
    _goal_ref: ($) => $.goal_id,
    _spinner_ref: ($) => $.spinner_id,
    _set_ref: ($) => $.set_name,

    // Lower precedence so keywords like 'has', 'ambient', etc. win over identifiers
    identifier: ($) => token(prec(-1, /[a-zA-Z0-9_\-:#]+/)),

    number: ($) => /-?\d+/,
    pos_int: ($) => /[1-9]\d*/,
    score_threshold: ($) => /\d+(?:\.\d+)?/,

    // Strings: single-line '…' and "…", multi-line """…""" and '''…''', and raw r#"…"#
    string: ($) =>
      choice(
        token(/\"([^\"\\\n]|\\.)*\"/),
        token(/'([^'\\\n]|\\.)*'/),
        token(
          seq(
            '"""',
            repeat(choice(/[^\"]/, /\"[^\"]/, /\"\"[^\"]/, /\\./)),
            '"""',
          ),
        ),
        token(
          seq("'''", repeat(choice(/[^']/, /'[^']/, /''[^']/, /\\./)), "'''"),
        ),
        token(seq('r#"', repeat(choice(/[^\"]/, /\"[^#]/)), '"#')),
      ),

    boolean: ($) => choice("true", "false"),

    //
    //
    //
    //
    //
    // ROOM SET DECLARATIONS
    //
    //
    //
    //
    //
    set_decl: ($) =>
      seq(
        "let",
        "set",
        field("name", $.set_name),
        "=",
        field("room_list", $.set_list),
      ),
    set_list: ($) => seq("(", sep1($._room_ref, ","), ")"),

    //
    //
    //
    //
    //
    // GAME DEFINITIONS
    //
    //
    //
    //
    //
    game_def: ($) => seq("game", $.game_block),
    game_block: ($) => seq("{", repeat($._game_stmt), "}"),
    _game_stmt: ($) =>
      choice(
        $.game_title,
        $.game_intro,
        $.game_slug,
        $.game_author,
        $.game_version,
        $.game_blurb,
        $.game_player,
        $.game_scoring,
      ),
    game_title: ($) => seq("title", field("title", $.entity_name)),
    game_slug: ($) => seq("slug", field("slug", $.string)),
    game_author: ($) => seq("author", field("author", $.string)),
    game_version: ($) => seq("version", field("version", $.string)),
    game_blurb: ($) => seq("blurb", field("blurb", $.string)),
    game_intro: ($) => seq("intro", field("intro", $.entity_desc)),
    game_player: ($) => seq("player", $.player_block),
    player_block: ($) => seq("{", repeat($._player_stmt), "}"),
    _player_stmt: ($) =>
      choice($.player_name, $.player_desc, $.player_max_hp, $.player_start),
    player_name: ($) => seq("name", field("name", $.entity_name)),
    player_desc: ($) =>
      seq(choice("desc", "description"), field("description", $.entity_desc)),
    player_max_hp: ($) => seq("max_hp", field("max_hp", $.pos_int)),
    player_start: ($) => seq("start", "room", field("room_id", $._room_ref)),
    game_scoring: ($) => seq("scoring", $.scoring_block),
    scoring_block: ($) => seq("{", repeat($._scoring_stmt), "}"),
    _scoring_stmt: ($) => choice($.scoring_title, $.scoring_rank),
    scoring_title: ($) => seq("report_title", field("title", $.entity_name)),
    scoring_rank: ($) =>
      seq(
        "rank",
        field("threshold", $.score_threshold),
        field("title", $.entity_name),
        field("description", $.entity_desc),
      ),

    //
    //
    //
    //
    //
    // ROOM DEFINITIONS
    //
    //
    //
    //
    //
    room_def: ($) => seq("room", field("room_id", $.room_id), $.room_block),

    room_block: ($) => seq("{", repeat($._room_stmt), "}"),

    _room_stmt: ($) =>
      choice(
        $.room_name,
        $.room_desc,
        $.room_visited,
        $.room_exit,
        $.ovl_flag_binary,
        $.ovl_presence_pair,
        $.ovl_npc_state_set,
        $.overlay_stmt,
      ),

    room_name: ($) => seq("name", field("name", $.entity_name)),

    room_desc: ($) =>
      seq(choice("desc", "description"), field("description", $.entity_desc)),

    room_visited: ($) =>
      seq("visited", field("visited", alias($.boolean, $.room_visited))),

    // ----------------- room exits ----------------
    room_exit: ($) =>
      seq(
        "exit",
        field("dir", $.exit_dir),
        "->",
        field("dest", $._room_ref),
        optional($.exit_block),
      ),

    exit_block: ($) => seq("{", repeat1(seq($.exit_stmt, optional(","))), "}"),
    exit_stmt: ($) =>
      choice(
        $.required_items_stmt,
        $.required_flags_stmt,
        $.barred_stmt,
        "hidden",
        "locked",
      ),
    required_items_stmt: ($) =>
      seq("required_items", "(", sep1(field("item_id", $._item_ref), ","), ")"),
    required_flags_stmt: ($) =>
      seq(
        "required_flags",
        "(",
        sep1(field("flag_name", $._flag_ref), ","),
        ")",
      ),
    barred_stmt: ($) =>
      seq("barred", field("msg", alias($.string, $.player_message))),

    // --------------------- room overlays ---------------------
    overlay_stmt: ($) => seq("overlay", "if", $._ovl_cond_list, $.ovl_block),

    ovl_block: ($) => seq("{", $.ovl_text_stmt, "}"),

    ovl_text_stmt: ($) =>
      seq("text", field("text", alias($.string, $.ovl_text))),

    _ovl_cond_list: ($) =>
      choice(
        $._ovl_cond,
        sep1($._ovl_cond, ","),
        seq("(", sep1($._ovl_cond, ","), ")"),
      ),

    _ovl_cond: ($) =>
      choice(
        $.ovl_flag_status,
        $.ovl_item_presence,
        $.ovl_item_posession,
        $.ovl_npc_presence,
        $.ovl_npc_state,
        $.ovl_item_in_room,
      ),

    ovl_flag_status: ($) =>
      seq(
        "flag",
        choice("set", "unset", "complete"),
        field("flag_name", $._flag_ref),
      ),

    ovl_item_presence: ($) =>
      seq("item", choice("present", "absent"), field("item_id", $._item_ref)),

    ovl_item_posession: ($) =>
      seq(
        "player",
        choice("has", "missing"),
        "item",
        field("item_id", $._item_ref),
      ),

    ovl_npc_presence: ($) =>
      seq("npc", choice("present", "absent"), field("npc_id", $._npc_ref)),

    ovl_npc_state: ($) =>
      seq(
        "npc",
        "in",
        "state",
        field("npc_id", $._npc_ref),
        field("npc_state", $.npc_state),
      ),

    npc_state: ($) => choice($.npc_state_builtin, $.npc_state_custom),

    npc_state_builtin: ($) =>
      choice("normal", "happy", "bored", "tired", "sad", "mad"),

    npc_state_custom: ($) =>
      seq(
        "custom",
        choice(
          field("custom_state", $.custom_state),
          seq("(", field("custom_state", $.custom_state), ")"),
        ),
      ),

    ovl_item_in_room: ($) =>
      seq(
        "item",
        "in",
        "room",
        field("item_id", $._item_ref),
        field("room_id", $._room_ref),
      ),

    // ---------------- overlay flag sets --------------------
    ovl_flag_binary: ($) =>
      seq(
        "overlay",
        "if",
        "flag",
        field("flag_name", $._flag_ref),
        $.flag_binary_block,
      ),
    flag_binary_block: ($) =>
      seq(
        "{",
        "set",
        field("set_text", alias($.string, $.ovl_text)),
        "unset",
        field("unset_text", alias($.string, $.ovl_text)),
        "}",
      ),

    ovl_presence_pair: ($) =>
      seq(
        "overlay",
        "if",
        choice(
          seq("item", field("item_id", $._item_ref)),
          seq("npc", field("npc_id", $._npc_ref)),
        ),
        $.presence_pair_block,
      ),

    presence_pair_block: ($) =>
      seq(
        "{",
        "present",
        field("present_text", alias($.string, $.ovl_text)),
        "absent",
        field("absent_text", alias($.string, $.ovl_text)),
        "}",
      ),

    ovl_npc_state_set: ($) =>
      seq(
        "overlay",
        "if",
        "npc",
        field("npc_id", $._npc_ref),
        "here",
        $.npc_state_set_block,
      ),
    npc_state_set_block: ($) => seq("{", repeat1($.npc_state_set_line), "}"),
    npc_state_set_line: ($) =>
      seq(
        choice($.npc_state_builtin, $.npc_state_set_custom),
        field("text", alias($.string, $.ovl_text)),
      ),
    npc_state_set_custom: ($) =>
      seq("custom", "(", field("state", $.custom_state), ")"),

    //
    //
    //
    //
    //
    // ITEM DEFINITIONS
    //
    //
    //
    //
    //
    item_def: ($) => seq("item", field("item_id", $.item_id), $.item_block),
    item_block: ($) => seq("{", repeat($._item_stmt), "}"),
    _item_stmt: ($) =>
      choice(
        $.item_name_stmt,
        $.item_desc_stmt,
        $.item_loc_stmt,
        $.item_movability_stmt,
        $.item_ability_stmt,
        $.item_text_stmt,
        $.item_container_stmt,
        $.item_requires_stmt,
        $.item_consumable_stmt,
      ),
    item_name_stmt: ($) => seq("name", field("item_name", $.entity_name)),
    item_desc_stmt: ($) =>
      seq(
        choice("desc", "description"),
        field("item_description", $.entity_desc),
      ),
    item_loc_stmt: ($) => seq("location", $.item_location),
    item_location: ($) =>
      choice(
        seq("inventory", "player"),
        seq("room", field("room_id", $._room_ref)),
        seq("chest", field("chest_id", $._item_ref)),
        seq("npc", field("npc_id", $._npc_ref)),
        seq("nowhere", field("spawn_note", alias($.string, $.dev_note))),
      ),
    movability: ($) =>
      choice(
        "free",
        seq("fixed", field("note", alias($.string, $.dev_note))),
        seq("restricted", field("note", alias($.string, $.dev_note))),
      ),
    item_movability_stmt: ($) =>
      seq("movability", field("movability", $.movability)),
    item_ability_stmt: ($) =>
      seq(
        "ability",
        field("ability", $.item_ability),
        // ability_target = id of something targeted by this ability e.g. unlock <which_item>
        optional(field("target_id", $._item_ref)),
      ),
    item_text_stmt: ($) =>
      seq("text", field("detail_text", alias($.string, $.item_detail_text))),
    item_container_stmt: ($) => seq("container", "state", $.container_state),
    container_state: ($) =>
      choice(
        "open",
        "closed",
        "locked",
        "transparentOpen",
        "transparentClosed",
        "transparentLocked",
        "none",
      ),
    item_requires_stmt: ($) =>
      seq(
        "requires",
        field("ability", $.item_ability),
        "to",
        field("interaction", $.item_interaction),
      ),
    item_consumable_stmt: ($) => seq("consumable", $.consumable_block),
    consumable_block: ($) => seq("{", repeat($._consumable_stmt), "}"),
    _consumable_stmt: ($) =>
      choice(
        $.consumable_uses,
        $.consumable_consume_on,
        $.consumable_when_consumed,
      ),
    consumable_uses: ($) => seq("uses_left", field("uses_left", $.pos_int)),
    consumable_consume_on: ($) =>
      seq("consume_on", "ability", field("ability", $.item_ability)),
    consumable_when_consumed: ($) => seq("when_consumed", $.when_consumed_opt),
    when_consumed_opt: ($) =>
      choice(
        "despawn",
        seq(
          "replace",
          choice(
            seq("inventory", field("item_id", $._item_ref)),
            seq("current", "room", field("item_id", $._item_ref)),
          ),
        ),
      ),

    //
    //
    //
    //
    // NPC DEFINITIONS
    //
    //
    //
    //
    npc_def: ($) => seq("npc", field("npc_id", $.npc_id), $.npc_block),
    npc_block: ($) => seq("{", repeat($._npc_stmt), "}"),
    _npc_stmt: ($) =>
      choice(
        $.npc_name_stmt,
        $.npc_desc_stmt,
        $.npc_max_hp_stmt,
        $.npc_loc_stmt,
        $.npc_state_stmt,
        $.npc_movement_stmt,
        $.npc_dialogue_block,
      ),
    npc_name_stmt: ($) => seq("name", field("npc_name", $.entity_name)),
    npc_desc_stmt: ($) =>
      seq(
        choice("desc", "description"),
        field("npc_description", $.entity_desc),
      ),
    npc_max_hp_stmt: ($) => seq("max_hp", field("max_hp", $.pos_int)),
    npc_loc_stmt: ($) => seq("location", $.npc_location),
    npc_location: ($) =>
      choice(
        seq("nowhere", field("spawn_note", alias($.string, $.dev_note))),
        seq("room", field("room_id", $._room_ref)),
      ),
    npc_state_stmt: ($) => seq("state", $.npc_state),

    npc_movement_stmt: ($) =>
      seq(
        "movement",
        $.movement_type,
        "rooms",
        $.room_list,
        optional($.timing_stmt),
        optional($.active_stmt),
        optional($.loop_stmt),
      ),
    movement_type: ($) => choice("route", "random"),
    room_list: ($) => seq("(", sep1(field("room_id", $._room_ref), ","), ")"),
    timing_stmt: ($) =>
      seq("timing", field("timing", alias($.identifier, $.timing))),
    active_stmt: ($) => seq("active", field("active", $.boolean)),
    loop_stmt: ($) => seq("loop", field("loop", $.boolean)),

    npc_dialogue_block: ($) =>
      seq(
        "dialogue",
        $.npc_state,
        "{",
        repeat1(field("dialogue", alias($.string, $.npc_dialogue))),
        "}",
      ),

    //
    //
    //
    //
    //
    // TRIGGER DEFINITIONS
    //
    //
    //
    //
    //
    trigger_def: ($) =>
      seq(
        "trigger",
        field("name", alias($.string, $.entity_name)),
        repeat(choice(field("once", $.only_once_kw), $.trigger_note)),
        "when",
        $.when_cond,
        $.trigger_block,
      ),
    only_once_kw: ($) => seq("only", "once"),
    trigger_note: ($) =>
      seq("note", field("trigger_note", alias($.string, $.dev_note))),

    // All "when ..." event variants (do not hide when_cond itself)
    _when_event: ($) =>
      choice(
        $.always_event,
        $.player_death,
        $.npc_death,
        $.enter_room,
        $.take_item,
        $.touch_item,
        $.talk_to_npc,
        $.open_item,
        $.leave_room,
        $.look_at_item,
        $.use_item,
        $.give_to_npc,
        $.use_item_on_item,
        $.act_on_item,
        $.take_from_npc,
        $.insert_item_into,
        $.drop_item,
        $.unlock_item,
        $.ingest_item,
      ),

    // All condition variants (keep trigger_cond visible)
    _trigger_cond_atom: ($) =>
      choice(
        $.cond_any_group,
        $.cond_all_group,
        $.cond_has_flag,
        $.cond_missing_flag,
        $.cond_has_item,
        $.cond_missing_item,
        $.cond_visited_room,
        $.cond_flag_in_progress,
        $.cond_flag_complete,
        $.cond_with_npc,
        $.cond_npc_has_item,
        $.cond_npc_in_state,
        $.cond_player_in_room,
        $.cond_container_has_item,
        $.cond_chance,
        $.cond_ambient,
        $.cond_in_rooms,
      ),

    // "when" conditions / triggering events
    when_cond: ($) => $._when_event,
    always_event: ($) => "always",
    player_death: ($) => seq("player", "dies"),
    npc_death: ($) => seq("npc", field("npc_id", $._npc_ref), "dies"),
    enter_room: ($) => seq("enter", "room", field("room_id", $._room_ref)),
    leave_room: ($) => seq("leave", "room", field("room_id", $._room_ref)),
    take_item: ($) => seq("take", "item", field("item_id", $._item_ref)),
    touch_item: ($) => seq("touch", "item", field("item_id", $._item_ref)),
    drop_item: ($) => seq("drop", "item", field("item_id", $._item_ref)),
    open_item: ($) => seq("open", "item", field("item_id", $._item_ref)),
    unlock_item: ($) => seq("unlock", "item", field("item_id", $._item_ref)),
    talk_to_npc: ($) => seq("talk", "to", "npc", field("npc_id", $._npc_ref)),
    look_at_item: ($) =>
      seq("look", "at", "item", field("item_id", $._item_ref)),
    use_item: ($) =>
      seq(
        "use",
        "item",
        field("item_id", $._item_ref),
        "ability",
        field("ability", $.item_ability),
      ),
    give_to_npc: ($) =>
      seq(
        "give",
        "item",
        field("item_id", $._item_ref),
        "to",
        "npc",
        field("npc_id", $._npc_ref),
      ),
    use_item_on_item: ($) =>
      seq(
        "use",
        "item",
        field("tool_id", $._item_ref),
        "on",
        "item",
        field("target_id", $._item_ref),
        "interaction",
        field("interaction", $.item_interaction),
      ),
    act_on_item: ($) =>
      seq(
        "act",
        field("action", $.item_interaction),
        "on",
        "item",
        field("item_id", $._item_ref),
      ),
    take_from_npc: ($) =>
      seq(
        "take",
        "item",
        field("item_id", $._item_ref),
        "from",
        "npc",
        field("npc_id", $._npc_ref),
      ),
    insert_item_into: ($) =>
      seq(
        "insert",
        "item",
        field("item_id", $._item_ref),
        "into",
        "item",
        field("item_id", $._item_ref),
      ),
    ingest_item: ($) =>
      seq(
        choice("drink", "eat", "inhale"),
        "item",
        field("item_id", $._item_ref),
      ),

    // start trigger action block, with if and do statements
    trigger_block: ($) => seq("{", repeat1($._trigger_stmt), "}"),
    _trigger_stmt: ($) => choice($.do_action, $.cond_block),

    cond_block: ($) =>
      seq("if", $.trigger_cond, "{", repeat1($.do_action), "}"),
    trigger_cond: ($) => $._trigger_cond_atom,

    cond_any_group: ($) => seq("any", "(", sep1($.trigger_cond, ","), ")"),
    cond_all_group: ($) => seq("all", "(", sep1($.trigger_cond, ","), ")"),
    cond_has_flag: ($) => seq("has", "flag", field("flag_name", $._flag_ref)),
    cond_missing_flag: ($) =>
      seq("missing", "flag", field("flag_name", $._flag_ref)),
    cond_has_item: ($) => seq("has", "item", field("item_id", $._item_ref)),
    cond_missing_item: ($) =>
      seq("missing", "item", field("item_id", $._item_ref)),
    cond_visited_room: ($) =>
      seq("has", "visited", "room", field("room_id", $._room_ref)),
    cond_flag_in_progress: ($) =>
      seq("flag", "in", "progress", field("flag_name", $._flag_ref)),
    cond_flag_complete: ($) =>
      seq("flag", "complete", field("flag_name", $._flag_ref)),
    cond_with_npc: ($) => seq("with", "npc", field("npc_id", $._npc_ref)),
    cond_npc_has_item: ($) =>
      seq(
        "npc",
        "has",
        "item",
        field("npc_id", $._npc_ref),
        field("item_id", $._item_ref),
      ),
    cond_npc_in_state: ($) =>
      seq(
        "npc",
        "in",
        "state",
        field("npc_id", $._npc_ref),
        field("state", $.custom_state),
      ),
    cond_player_in_room: ($) =>
      seq("player", "in", "room", field("room_id", $._room_ref)),
    cond_container_has_item: ($) =>
      seq(
        "container",
        field("container_id", $._item_ref),
        "has",
        "item",
        field("item_id", $._item_ref),
      ),
    cond_chance: ($) => seq("chance", field("pct", $.pos_int), "%"),
    cond_ambient: ($) =>
      seq(
        "ambient",
        field("spinner", $._spinner_ref),
        // If present, prefer consuming commas as part of the inner room list
        // when nested inside grouped conditions like any(...) or all(...).
        optional(
          prec.left(
            seq("in", "rooms", sep1(field("room_id", $._room_ref), ",")),
          ),
        ),
      ),
    // Prefer consuming commas as part of the inner room list when nested
    // inside grouped conditions like any(...) or all(...). Use a restricted
    // identifier that excludes reserved keywords, so outer conditions like
    // 'has flag …' are not swallowed into the room list.
    cond_in_rooms: ($) =>
      prec.left(seq("in", "rooms", sep1(field("room_id", $._room_ref), ","))),

    do_action: ($) => seq("do", optional($.priority_clause), $._action_type),
    priority_clause: ($) =>
      seq("priority", field("display_priority", $.number)),
    _action_type: ($) =>
      choice(
        $.action_modify_item,
        $.action_modify_room,
        $.action_modify_npc,
        $.action_show,
        $.action_add_wedge,
        $.action_add_seq,
        $.action_replace_item,
        $.action_replace_drop_item,
        $.action_add_flag,
        $.action_reset_flag,
        $.action_remove_flag,
        $.action_advance_flag,
        $.action_spawn_room,
        $.action_spawn_container,
        $.action_spawn_inventory,
        $.action_spawn_current_room,
        $.action_spawn_npc_into_room,
        $.action_despawn_npc,
        $.action_despawn_item,
        $.action_award_points,
        $.action_lock_item,
        $.action_unlock_item,
        $.action_lock_exit,
        $.action_unlock_exit,
        $.action_reveal_exit,
        $.action_push_player,
        $.action_set_item_desc,
        $.action_set_item_movability,
        $.action_npc_random_dialogue,
        $.action_npc_says,
        $.action_npc_refuse_item,
        $.action_set_npc_active,
        $.action_set_npc_state,
        $.action_damage_player,
        $.action_heal_player,
        $.action_remove_player_effect,
        $.action_damage_npc,
        $.action_heal_npc,
        $.action_remove_npc_effect,
        $.action_deny_read,
        $.action_restrict_item,
        $.action_give_to_player,
        $.action_set_barred_msg,
        $.action_set_container_state,
        $.action_spinner_msg,
        $.action_schedule_in_or_on,
        $.action_schedule_in_if,
      ),
    action_modify_item: ($) =>
      seq("modify", "item", field("item_id", $._item_ref), $.item_patch_block),
    action_modify_room: ($) =>
      seq("modify", "room", field("room_id", $._room_ref), $.room_patch_block),
    action_modify_npc: ($) =>
      seq("modify", "npc", field("npc_id", $._npc_ref), $.npc_patch_block),
    item_patch_block: ($) => seq("{", repeat1($._item_patch_stmt), "}"),
    _item_patch_stmt: ($) =>
      choice(
        $.item_patch_name,
        $.item_patch_desc,
        $.item_patch_text,
        $.item_patch_movability,
        $.item_patch_container_state,
        $.item_patch_add_ability,
        $.item_patch_remove_ability,
      ),
    item_patch_name: ($) => seq("name", field("name", $.entity_name)),
    item_patch_desc: ($) =>
      seq(choice("desc", "description"), field("description", $.entity_desc)),
    item_patch_text: ($) => seq("text", field("text", $.string)),
    item_patch_movability: ($) =>
      seq("movability", field("movability", $.movability)),
    item_patch_container_state: ($) =>
      seq("container", "state", field("container_state", $.off_or_state)),
    off_or_state: ($) => choice("off", $.container_state),
    item_patch_add_ability: ($) =>
      seq("add", "ability", field("ability", $.patch_ability)),
    item_patch_remove_ability: ($) =>
      seq("remove", "ability", field("ability", $.patch_ability)),
    patch_ability: ($) =>
      seq(
        field("ability_name", alias($.identifier, $.ability_name)),
        optional($.ability_target),
      ),
    ability_target: ($) => seq("(", field("item", $._item_ref), ")"),
    room_patch_block: ($) => seq("{", repeat1($._room_patch_stmt), "}"),
    _room_patch_stmt: ($) =>
      choice(
        $.room_patch_name,
        $.room_patch_desc,
        $.room_patch_remove_exit,
        $.room_patch_add_exit,
      ),
    room_patch_name: ($) => seq("name", field("name", $.entity_name)),
    room_patch_desc: ($) =>
      seq(choice("desc", "description"), field("description", $.entity_desc)),
    room_patch_remove_exit: ($) =>
      seq("remove", "exit", field("destination", $._room_ref)),
    room_patch_add_exit: ($) =>
      seq(
        "add",
        "exit",
        field("direction", $.exit_dir),
        "->",
        field("destination", $._room_ref),
        optional($.exit_block),
      ),
    npc_patch_block: ($) => seq("{", repeat1($._npc_patch_stmt), "}"),
    _npc_patch_stmt: ($) =>
      choice(
        $.npc_patch_name,
        $.npc_patch_desc,
        $.npc_patch_state,
        $.npc_patch_add_line,
        $.npc_patch_route,
        $.npc_patch_random_rooms,
        $.npc_patch_timing_every,
        $.npc_patch_timing_on,
        $.npc_patch_active,
        $.npc_patch_loop,
      ),
    npc_patch_name: ($) => seq("name", field("name", $.entity_name)),
    npc_patch_desc: ($) =>
      seq(choice("desc", "description"), field("description", $.entity_desc)),
    npc_patch_state: ($) =>
      seq("state", field("state", choice($.npc_state, $.custom_state))),
    npc_patch_add_line: ($) =>
      seq(
        "add",
        "line",
        field("dialogue", alias($.string, $.npc_dialogue)),
        "to",
        "state",
        field("state", choice($.npc_state, $.custom_state)),
      ),
    npc_patch_route: ($) =>
      seq("route", "(", sep1(field("room_id", $._room_ref), ","), ")"),
    npc_patch_random_rooms: ($) =>
      seq(
        "random",
        "rooms",
        "(",
        sep1(field("room_id", $._room_ref), ","),
        ")",
      ),
    npc_patch_timing_every: ($) =>
      seq("timing", "every", field("interval", $.pos_int), "turns"),
    npc_patch_timing_on: ($) =>
      seq("timing", "on", "turn", field("turn", $.pos_int)),
    npc_patch_active: ($) => seq("active", field("active", $.boolean)),
    npc_patch_loop: ($) => seq("loop", field("loop", $.boolean)),

    action_show: ($) =>
      seq("show", field("text", alias($.string, $.player_message))),
    action_add_wedge: ($) =>
      seq(
        "add",
        "wedge",
        field("text", alias($.string, $.wedge_text)),
        optional(seq("width", $.number)),
        "spinner",
        field("spinner", $._spinner_ref),
      ),
    action_add_seq: ($) =>
      seq(
        "add",
        "seq",
        "flag",
        field("flag_name", $.flag_name),
        optional(seq("limit", $.number)),
      ),
    action_replace_item: ($) =>
      seq(
        "replace",
        "item",
        field("item_id", $._item_ref),
        "with",
        field("item_id", $._item_ref),
      ),
    action_replace_drop_item: ($) =>
      seq(
        "replace",
        "drop",
        "item",
        field("item_id", $._item_ref),
        "with",
        field("item_id", $._item_ref),
      ),
    action_add_flag: ($) => seq("add", "flag", field("flag", $.flag_name)),
    action_reset_flag: ($) => seq("reset", "flag", field("flag", $._flag_ref)),
    action_remove_flag: ($) =>
      seq("remove", "flag", field("flag", $._flag_ref)),
    action_advance_flag: ($) =>
      seq("advance", "flag", field("flag", $._flag_ref)),
    spawn_action_stem: ($) =>
      seq("spawn", "item", field("item_id", $._item_ref)),
    action_spawn_room: ($) =>
      seq($.spawn_action_stem, "into", "room", field("room", $._room_ref)),
    action_spawn_container: ($) =>
      seq(
        $.spawn_action_stem,
        choice("into", "in"),
        "container",
        field("container_id", $._item_ref),
      ),
    action_spawn_inventory: ($) => seq($.spawn_action_stem, "in", "inventory"),
    action_spawn_current_room: ($) =>
      seq($.spawn_action_stem, "in", "current", "room"),
    action_spawn_npc_into_room: ($) =>
      seq(
        "spawn",
        "npc",
        field("npc_id", $._npc_ref),
        "into",
        "room",
        field("room_id", $._room_ref),
      ),
    action_despawn_npc: ($) =>
      seq("despawn", "npc", field("npc_id", $._npc_ref)),
    action_despawn_item: ($) =>
      seq("despawn", "item", field("item_id", $._item_ref)),
    action_award_points: ($) =>
      seq(
        "award",
        "points",
        field("points", $.number),
        "reason",
        field("award_reason", $.string),
      ),
    action_lock_item: ($) => seq("lock", "item", field("item_id", $._item_ref)),
    action_unlock_item: ($) =>
      seq("unlock", "item", field("item_id", $._item_ref)),
    action_lock_exit: ($) =>
      seq(
        "lock",
        "exit",
        "from",
        field("room_id", $._room_ref),
        "direction",
        field("direction", $.exit_dir),
      ),
    action_unlock_exit: ($) =>
      seq(
        "unlock",
        "exit",
        "from",
        field("room_id", $._room_ref),
        "direction",
        field("direction", $.exit_dir),
      ),
    action_reveal_exit: ($) =>
      seq(
        "reveal",
        "exit",
        "from",
        field("from_room", $._room_ref),
        "to",
        field("to_room", $._room_ref),
        "direction",
        field("direction", $.exit_dir),
      ),
    action_push_player: ($) =>
      seq("push", "player", "to", field("room_id", $._room_ref)),
    action_set_item_desc: ($) =>
      seq(
        "set",
        "item",
        "description",
        field("item_id", $._item_ref),
        field("text", alias($.string, $.entity_desc)),
      ),
    action_set_item_movability: ($) =>
      seq(
        "set",
        "item",
        "movability",
        field("item_id", $._item_ref),
        field("movability", $.movability),
      ),
    action_npc_random_dialogue: ($) =>
      seq("npc", "random", "dialogue", field("npc_id", $._npc_ref)),
    action_npc_says: ($) =>
      seq(
        "npc",
        "says",
        field("npc_id", $._npc_ref),
        field("text", alias($.string, $.quote)),
      ),
    action_npc_refuse_item: ($) =>
      seq(
        "npc",
        "refuse",
        "item",
        field("npc_id", $._npc_ref),
        field("reason", alias($.string, $.player_message)),
      ),
    action_set_npc_active: ($) =>
      seq("set", "npc", "active", field("npc_id", $._npc_ref), $.boolean),
    action_set_npc_state: ($) =>
      seq(
        "set",
        "npc",
        "state",
        field("npc_id", $._npc_ref),
        field("state", $.custom_state),
      ),
    action_damage_player: ($) =>
      seq(
        "damage",
        "player",
        field("amount", $.number),
        optional($.effect_duration_clause),
        "cause",
        field("cause", alias($.string, $.player_message)),
      ),
    action_heal_player: ($) =>
      seq(
        "heal",
        "player",
        field("amount", $.number),
        optional($.effect_duration_clause),
        "cause",
        field("cause", alias($.string, $.player_message)),
      ),
    action_remove_player_effect: ($) =>
      seq(
        "remove",
        "player",
        "effect",
        field("effect", alias($.string, $.effect_name)),
      ),
    action_damage_npc: ($) =>
      seq(
        "damage",
        "npc",
        field("npc_id", $._npc_ref),
        field("amount", $.number),
        optional($.effect_duration_clause),
        "cause",
        field("cause", alias($.string, $.player_message)),
      ),
    action_heal_npc: ($) =>
      seq(
        "heal",
        "npc",
        field("npc_id", $._npc_ref),
        field("amount", $.number),
        optional($.effect_duration_clause),
        "cause",
        field("cause", alias($.string, $.player_message)),
      ),
    action_remove_npc_effect: ($) =>
      seq(
        "remove",
        "npc",
        field("npc_id", $._npc_ref),
        "effect",
        field("effect", alias($.string, $.effect_name)),
      ),
    effect_duration_clause: ($) =>
      seq("for", field("duration", $.pos_int), "turns"),
    action_deny_read: ($) =>
      seq("deny", "read", field("reason", alias($.string, $.player_message))),
    action_restrict_item: ($) =>
      seq("restrict", "item", field("item_id", $._item_ref)),
    action_give_to_player: ($) =>
      seq(
        "give",
        "item",
        field("item_id", $._item_ref),
        "to",
        "player",
        "from",
        "npc",
        field("npc_id", $._npc_ref),
      ),
    action_set_barred_msg: ($) =>
      seq(
        "set",
        "barred",
        "message",
        "from",
        field("room_id", $._room_ref),
        "to",
        field("room_id", $._room_ref),
        field("msg", alias($.string, $.player_message)),
      ),
    action_set_container_state: ($) =>
      seq(
        "set",
        "container",
        "state",
        field("item_id", $._item_ref),
        $.container_state,
      ),
    action_spinner_msg: ($) =>
      seq("spinner", "message", field("spinner", $._spinner_ref)),

    // scheduler actions
    schedule_note: ($) =>
      seq("note", field("text", alias($.string, $.schedule_note_text))),
    retry_type: ($) =>
      seq(
        "onFalse",
        field(
          "policy",
          alias(
            choice("cancel", "retryNextTurn", seq("retryAfter", $.number)),
            $.retry_policy,
          ),
        ),
      ),
    action_schedule_in_or_on: ($) =>
      seq(
        "schedule",
        choice("in", "on"),
        field("turns", $.number),
        optional($.schedule_note),
        $.trigger_block,
      ),
    action_schedule_in_if: ($) =>
      seq(
        "schedule",
        choice("in", "on"),
        field("turns", $.number),
        "if",
        $.trigger_cond,
        optional($.retry_type),
        optional($.schedule_note),
        $.trigger_block,
      ),

    //
    //
    //
    //
    //
    // SPINNER DEFINITIONS
    //
    //
    //
    //
    //
    spinner_def: ($) =>
      seq("spinner", field("name", $.spinner_id), $.spinner_block),
    spinner_block: ($) => seq("{", repeat1($.spinner_stmt), "}"),
    spinner_stmt: ($) =>
      seq(
        "wedge",
        field("spinner_text", alias($.string, $.spinner_text)),
        optional(seq("width", field("width", $.pos_int))),
      ),

    //
    //
    //
    //
    //
    // GOAL DEFINITIONS
    //
    //
    //
    //
    //
    goal_def: ($) => seq("goal", field("goal_id", $.goal_id), $.goal_block),
    goal_block: ($) => seq("{", repeat1($._goal_stmt), "}"),
    _goal_stmt: ($) =>
      choice(
        $.goal_name_stmt,
        $.goal_desc_stmt,
        $.goal_group_stmt,
        $.goal_start_stmt,
        $.goal_done_stmt,
        $.goal_fail_stmt,
      ),
    goal_name_stmt: ($) =>
      seq("name", field("goal_name", alias($.string, $.entity_name))),
    goal_desc_stmt: ($) =>
      seq("desc", field("goal_description", alias($.string, $.entity_desc))),
    goal_group_stmt: ($) =>
      seq(
        "group",
        field(
          "goal_group",
          alias(choice("required", "optional", "status-effect"), $.goal_group),
        ),
      ),
    goal_start_stmt: ($) =>
      seq("start", "when", field("start_condition", $._goal_cond)),
    goal_done_stmt: ($) =>
      seq("done", "when", field("done_condition", $._goal_cond)),
    goal_fail_stmt: ($) =>
      seq("fail", "when", field("fail_condition", $._goal_cond)),
    _goal_cond: ($) =>
      choice(
        $.gc_has_flag,
        $.gc_missing_flag,
        $.gc_has_item,
        $.gc_reached_room,
        $.gc_goal_complete,
        $.gc_flag_progress,
        $.gc_flag_complete,
      ),
    gc_has_flag: ($) => seq("has", "flag", field("flag_name", $._flag_ref)),
    gc_missing_flag: ($) =>
      seq("missing", "flag", field("flag_name", $._flag_ref)),
    gc_has_item: ($) => seq("has", "item", field("item_id", $._item_ref)),
    gc_reached_room: ($) =>
      seq("reached", "room", field("room_id", $._room_ref)),
    gc_goal_complete: ($) =>
      seq("goal", "complete", field("goal_id", $._goal_ref)),
    gc_flag_progress: ($) =>
      seq("flag", "in", "progress", field("flag_name", $._flag_ref)),
    gc_flag_complete: ($) =>
      seq("flag", "complete", field("flag_name", $._flag_ref)),
  },
});

function sep1(rule, delimiter) {
  return seq(rule, repeat(seq(delimiter, rule)));
}
