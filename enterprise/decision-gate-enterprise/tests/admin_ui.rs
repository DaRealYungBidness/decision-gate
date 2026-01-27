// enterprise/decision-gate-enterprise/tests/admin_ui.rs
// ============================================================================
// Module: Admin UI Tests
// Description: Unit tests for admin UI scaffolding.
// Purpose: Validate HTML rendering output.
// ============================================================================

//! Admin UI unit tests.

use decision_gate_enterprise::admin_ui::render_admin_index;

#[test]
fn admin_ui_renders_nonempty_html() {
    let html = render_admin_index();
    assert!(!html.is_empty());
    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("<html"));
    assert!(html.contains("Decision Gate Admin"));
}
