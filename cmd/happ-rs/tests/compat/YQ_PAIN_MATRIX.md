# yq Pain Matrix (tracked for happ)

Snapshot date: 2026-03-01
Upstream: mikefarah/yq v4.52.4

## Purpose

This matrix tracks painful/unstable yq areas and links them to happ regression coverage.
Coverage can be:

- `covered`: explicit regression test exists and is green
- `partial`: behavior is constrained/guarded, but not fully equivalent to upstream yq behavior
- `gap`: known issue, not yet covered by automated regression in happ

## Matrix

| Category | Upstream issues | happ status | Regression refs |
|---|---|---|---|
| Merge semantics (`<<` single/seq/local override) | [#2434](https://github.com/mikefarah/yq/issues/2434), [#2524](https://github.com/mikefarah/yq/issues/2524), [#2502](https://github.com/mikefarah/yq/issues/2502) | covered | `spec_merge_single_map`, `spec_merge_sequence_precedence`, `spec_merge_local_override`, `spec_merge_inline_flow_map_order` |
| Plain `<<` vs quoted `"<<"` distinction | YAML merge key semantics | covered | `spec_quoted_merge_key_is_regular_key` |
| Anchors/aliases validity & failures | [#2406](https://github.com/mikefarah/yq/issues/2406), [#1694](https://github.com/mikefarah/yq/issues/1694), [#2040](https://github.com/mikefarah/yq/issues/2040) | partial | `parser_unknown_alias_must_error`, `parser_cycle_anchor_must_error_issue_2040`, `anchor_name_with_dash_underscore` |
| Duplicate keys validation | [#2228](https://github.com/mikefarah/yq/issues/2228) | covered | `parser_duplicate_keys_must_error_issue_2228` |
| Expression/key-iteration dropping values | [#2593](https://github.com/mikefarah/yq/issues/2593) | covered | `issue_2593_keys_iteration_keeps_all_items` |
| Helm-templated YAML parse failures | [#2270](https://github.com/mikefarah/yq/issues/2270) | covered (as expected parser error) | `parser_helm_template_must_error_issue_2270` |
| Block/folded scalar semantics | [#566](https://github.com/mikefarah/yq/issues/566), [#1093](https://github.com/mikefarah/yq/issues/1093) | covered | `block_scalar_preserves_leading_blank_line_issue_1093_shape`, `block_scalar_trailing_spaces_preserved_issue_566_shape`, `folded_scalar_basic` |
| Complex/special map keys | [#2403](https://github.com/mikefarah/yq/issues/2403), [#1323](https://github.com/mikefarah/yq/issues/1323) | covered | `complex_key_lookup_bracket_form_issue_2403_shape`, `key_with_asterisk_lookup_issue_1323_shape` |
| Multi-document stream behavior | [#1900](https://github.com/mikefarah/yq/issues/1900) | covered in CLI/web with `--doc-mode` / converter doc mode | `multi_document_stream_currently_rejected` (legacy parser contract), CLI doc-mode tests |
| Comment-preserving roundtrip | [#2600](https://github.com/mikefarah/yq/issues/2600), [#2578](https://github.com/mikefarah/yq/issues/2578), [#2516](https://github.com/mikefarah/yq/issues/2516) | gap | N/A (happ parser model does not preserve comments) |
| Line-number diagnostics accuracy | [#1956](https://github.com/mikefarah/yq/issues/1956) | gap | N/A |
| Very large YAML / perf | [#1215](https://github.com/mikefarah/yq/issues/1215) | gap | N/A (no stress benchmark gate yet) |

## Known technical limitation

Style-aware merge-key hinting now uses libyaml scalar style, so plain `<<` and quoted `"<<"` are handled differently.
Remaining limitation: complex/non-scalar mapping keys are tracked conservatively (`<complex-key>` path fallback).
