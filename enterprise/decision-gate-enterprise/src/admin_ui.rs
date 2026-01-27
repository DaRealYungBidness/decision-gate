// enterprise/decision-gate-enterprise/src/admin_ui.rs
// ============================================================================
// Module: Admin UI Scaffolding
// Description: Minimal HTML scaffolding for tenant admin UI.
// Purpose: Provide a starting point for Phase-1 management UI.
// ============================================================================

/// Returns a minimal HTML shell for the admin UI.
#[must_use]
pub const fn render_admin_index() -> &'static str {
    "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\"/>\n  <meta \
     name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>\n  <title>Decision Gate \
     Admin</title>\n</head>\n<body>\n  <h1>Decision Gate Admin</h1>\n  <p>Tenant management UI \
     scaffolding (Phase 1).</p>\n</body>\n</html>"
}
