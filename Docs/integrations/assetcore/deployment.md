<!--
Docs/integrations/assetcore/deployment.md
============================================================================
Document: DG + AssetCore Deployment Patterns
Description: Conceptual deployment topology notes for DG/ASC integration.
Purpose: Provide operator guidance without implying implementation coupling.
Dependencies:
  - Docs/architecture/decision_gate_assetcore_integration_contract.md
============================================================================
-->

# DG + AssetCore Deployment Patterns (Conceptual)

## Reference Topology
- DG MCP server runs as its own control-plane service.
- ASC read daemon runs as a separate world-state service.
- Integration layer handles ASC auth and maps principals to DG tool permissions.

## Deployment Notes
- **Separation of concerns**: DG controls decisions; ASC controls world-state.
- **Fail-closed integration**: Namespace and auth checks must fail closed.
- **Determinism first**: Ensure ASC read responses include anchors for replay.

## TODO (V1 Placeholder)
- Provide validated deployment recipes once reference deployments exist.
- Include security hardening guides (mTLS, audit log shipping, rate limits).
- Publish production-ready HA/control-plane diagrams.
