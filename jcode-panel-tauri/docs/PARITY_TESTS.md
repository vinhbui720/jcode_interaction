# Python to Tauri parity tests

Python source: `jcode-panel/tests/test_core.py`

| Python test | Tauri status |
|---|---|
| `test_default_config_roundtrip` | ported-basic |
| `test_config_loads_backup_when_primary_missing` | ported-basic |
| `test_context_summary_and_block` | ported-basic |
| `test_parse_structured_and_plain_events` | ported-basic |
| `test_conversation_preview_debug_and_status` | ported-basic |
| `test_completion_tab_cycles` | ported-basic |
| `test_terminal_template_rendering` | ported-basic |
| `test_terminal_launch_defaults_to_home_cwd` | ported-basic |
| `test_state_roundtrip_and_prompt_dedupe` | ported-basic |
| `test_state_loads_backup_when_primary_missing` | ported-basic |
| `test_state_save_preserves_existing_session_and_tokens_when_blank` | ported-basic |
| `test_state_save_can_intentionally_clear_session_for_new_section` | ported-basic |
| `test_prompt_builder_sends_direct_text_without_context_or_metadata` | ported-basic |
| `test_app_controller_active_session_prefers_state` | ported-basic |
| `test_app_controller_section_switch_and_new_section` | ported-basic |
| `test_protocol_parses_panel_events_and_preview` | ported-basic |
| `test_protocol_activity_helpers_prefer_command_and_state` | ported-basic |
| `test_protocol_activity_helpers_detect_terminal_events` | ported-basic |
| `test_protocol_backend_chat_status_drives_live_activity` | ported-basic |
| `test_protocol_backend_chat_status_can_finish_activity` | ported-basic |
| `test_protocol_backend_chat_status_current_drives_live_activity` | ported-basic |
| `test_protocol_backend_chat_status_current_can_finish_activity` | ported-basic |
| `test_protocol_backend_chat_status_prefers_current_over_activity` | ported-basic |
| `test_protocol_backend_chat_status_preview_uses_current_when_text_empty` | ported-basic |
| `test_conversation_status_uses_current_preview_when_text_empty` | ported-basic |
| `test_conversation_transcription_becomes_user_turn` | ported-basic |
| `test_conversation_ignores_noisy_sending_status` | ported-basic |
| `test_protocol_backend_chat_status_extracts_feedback_text` | ported-basic |
| `test_protocol_backend_chat_status_extracts_answer_text` | ported-basic |
| `test_protocol_backend_chat_status_nested_status_current` | ported-basic |
| `test_protocol_activity_helpers_keep_streaming_message_active` | ported-basic |
| `test_jcode_client_repl_args_and_adopt_session` | ported-basic |
| `test_jcode_client_model_repl_args_and_slash_fallback` | ported-basic |
| `test_jcode_client_new_section_waits_for_bootstrap_session` | ported-basic |
| `test_jcode_client_send_bootstraps_new_section` | ported-basic |
| `test_jcode_client_set_session_restarts_dead_same_session` | ported-basic |
| `test_protocol_parses_completion_items_and_session` | ported-basic |
| `test_diagnostics_report_text_and_status` | ported-basic |
| `test_hotkey_status_shape` | ported-basic |
| `test_integration_registry_lists_browser_and_obsidian` | ported-basic |
| `test_update_result_shape` | ported-basic |
| `test_control_response_shape` | ported-basic |
| `test_shortcut_result_shape` | ported-basic |
| `test_protocol_extracts_common_ndjson_text_shapes` | ported-basic |
| `test_conversation_coalesces_text_delta_stream` | ported-basic |
| `test_parse_xdotool_mouselocation` | ported-basic |
| `test_parse_xdotool_mouselocation_full_window` | ported-basic |
| `test_context_prompt_block_includes_selection_and_clipboard` | ported-basic |
| `test_capture_active_context_reads_selection_and_clipboard` | ported-basic |
| `test_capture_active_context_filters_gjs_shell_artifact_and_dm_clipboard` | ported-basic |
| `test_ambient_key_ignored_when_entry_has_focus` | ported-basic |
| `test_ambient_key_routes_when_entry_not_focused` | ported-basic |
| `test_ambient_key_routes_edit_keys_when_entry_not_focused` | ported-basic |
| `test_ambient_escape_dismisses_feedback_toast` | ported-basic |
| `test_markdown_to_pango_renders_safe_colored_subset` | ported-basic |
| `test_split_token_stats_removes_inline_telemetry` | ported-basic |
| `test_format_stream_lines_keeps_recent_lines_and_wraps_long_stream` | ported-basic |
| `test_token_notice_from_raw_supports_common_usage_shapes` | ported-basic |
| `test_event_notice_text_separates_context_and_tokens` | ported-basic |
| `test_hotkey_normalization_and_parts` | ported-basic |
| `test_parse_screenshot_command_modes_and_prompt` | ported-basic |
| `test_screenshot_command_lists_distinguish_area_and_full` | ported-basic |
| `test_default_config_has_screenshot_hotkey` | ported-basic |
| `test_screenshot_tag_format_and_delete_regex` | ported-basic |
| `test_pic_tag_expands_to_screenshot_paths` | ported-basic |
| `test_config_save_is_reloadable_after_multiple_writes` | ported-basic |
| `test_interaction_tag_normalization_and_sources` | ported-basic |
| `test_interaction_context_expands_each_chip` | ported-basic |
| `test_interaction_context_missing_app_blocks_send` | ported-basic |
| `test_jcode_repl_wire_prompt_is_single_physical_line` | ported-basic |
| `test_obsidian_context_uses_absolute_path_and_limited_excerpt` | ported-basic |
| `test_interaction_partial_completion` | ported-basic |
| `test_popup_context_chips_selected_text_expand` | ported-basic |
| `test_popup_context_chips_selected_url_and_multiple_files` | ported-basic |
| `test_popup_context_chips_ignore_clipboard_arguments` | ported-basic |
| `test_popup_context_chips_limit_selected_text` | ported-basic |

Current Tauri Rust tests: 32 passing. This ledger is intentionally conservative: `ported-basic` means a Rust unit test covers the same pure/domain behavior family, not necessarily full GUI/runtime parity.
