// Minimum fixed acceptance schedules. Additional named tests are welcome, but
// deleting one of these may not silently shrink the acceptance suite.
export const requiredCases = Object.freeze({
  wire: ['wire_lengths_and_ioctl_values','wire_header_literal_and_bounds','wire_known_read_pair','wire_binding_and_reserved_rejected','wire_actual_returned_bytes_not_capacity','wire_unknown_information_distinct_from_zero','wire_full_width_little_endian'],
  recorder: ['producer_65th_submit','producer_ring_drop_pair','producer_stop_late_completion','producer_duplicate_token','producer_invalid_snapshot_retirement','producer_pop_not_context_retirement','producer_counter_overflow','producer_last_ordinal_once','producer_last_id_once','producer_popped_and_skipped_overflow','producer_old_token_cannot_retire_reused_slot','producer_unarmed_cleanup_and_usage_veto','producer_consumer_loss_window','producer_skipped_window','producer_fifo_wrap'],
  driver: ['add_absent_no_attach','add_malformed_no_attach','add_mismatch_no_attach','add_exact_once','add_concurrent_exact_single_claim','add_create_fdo_failure','add_create_cdo_failure','add_link_failure','add_attach_failure','add_inline_dispatch_before_return','io_inline_success','io_inline_complete_pending_return','io_delayed_completion','io_cancel_and_error','observer_no_context_release_only','observer_stopped_release_only','context_retire_reuse_before_old_remove_release','remove_waits_for_tracked_completion','remove_waits_for_unobserved_completion','remove_waits_for_control_call_not_open_handle','cdo_close_after_fdo_deleted','start_lower_pending_bottom_up','start_lower_failure','cancel_remove_lower_pending_bottom_up','cancel_stop_lower_pending_bottom_up','spurious_cancels','query_stop_resource_free_forward','stop_preserves_late_callback_state','surprise_then_remove','remove_without_start','power_dispatch_irql_matrix','usage_0_through_7_true_false','usage_future_true_false','usage_noncanonical_boolean_true','usage_postdisplay_before_start','control_buffer_alias_and_truncation','control_second_fileobject_denied','control_cleanup_during_remove','cleanup_before_begin_no_session','begin_before_cleanup_stops_session','close_before_begin_does_not_poison_session','control_old_close_preserves_new_owner'],
});
export function verifySuite(suite, output) {
  const passed = [...output.matchAll(/^P04_CASE_PASS ([a-zA-Z0-9_]+)\r?$/gm)].map(m=>m[1]);
  const summary = [...output.matchAll(/^P04_SUITE_PASS suite=([a-z]+) cases=([0-9]+)\r?$/gm)];
  if (!Object.hasOwn(requiredCases,suite) || summary.length!==1 || summary[0][1]!==suite ||
      Number(summary[0][2])<=0 || Number(summary[0][2])!==passed.length ||
      new Set(passed).size!==passed.length || requiredCases[suite].some(name=>!passed.includes(name))) {
    throw new Error('P04_NONZERO_NAMED_PROOF_MISSING');
  }
  return passed.length;
}
