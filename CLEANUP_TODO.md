# Cleanup TODO

Minor cleanup items that can be addressed in a future commit:

## Code Cleanup
- [ ] Remove debug `eprintln!` statements in analyze_document
- [ ] Remove unused `get_room_id_at_position` function (replaced by `get_symbol_at_position`)  
- [ ] Remove unused `offset_to_position` helper if not needed

## Note
These items don't affect functionality - the LSP works perfectly with them present.
They're just minor code quality improvements.

