# The format 2 corpus

Items as format 2 wrote them, with the values they parsed to **then**.

Format 2 stopped being current when format 3 arrived, and nothing here changes
again. If a case looks wrong, it was wrong then, and that is what a reader in
ten years will meet — the fix is a new case in the current corpus, not an edit
here. A digest in `tests/format.rs` holds it to that.

Format 2 is the one where milestones became items. Format 3 moved the
declaration that a type groups work off a `[[field]]` and onto the type, and
changed no item file at all — which is why these parse identically under the
current build.
