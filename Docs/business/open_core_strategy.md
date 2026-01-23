<!--
Docs/business/open_core_strategy.md
============================================================================
Document: Decision Gate Open Core Strategy
Description: Business strategy for open core positioning, monetization, and
             market segmentation.
Purpose: Define what stays open, what we charge for, and how to achieve
         reference implementation status.
Dependencies:
  - Docs/roadmap/decision_gate_mcp_roadmap.md
  - Docs/security/threat_model.md
  - Asset Core integration (external repository)
============================================================================
-->

# Decision Gate Open Core Strategy

## Executive Summary

**Vision**: Decision Gate becomes the global reference implementation for
deterministic AI decision governance—the standard way agentic systems handle
evidence-backed disclosure control.

**Strategy**: Permissive open core (Apache 2.0) establishes credibility and
adoption; revenue flows through Asset Core integration, enterprise features,
and professional services.

**Positioning**: Build to defense/government standards. Let quality propagate
to easier domains. "We built for defense. You get defense-grade without the
hassle."

---

## Part 1: The Open Core

### 1.1 Core Thesis

Decision Gate answers questions the entire LLM/agent ecosystem cannot:

- **"Should this data be disclosed right now?"** — Evidence-backed gates
- **"What evidence justifies this decision?"** — Provider queries with anchors
- **"Can I prove what happened, when, why, to auditors?"** — Runpacks with manifests
- **"Can I replay this decision offline with identical results?"** — RFC 8785 determinism

No existing tool combines these capabilities. Policy engines (OPA, Cedar) lack
evidence anchoring and audit trails. Workflow engines (Temporal, Airflow) lack
three-valued logic and decision focus. AI safety tools (Guardrails AI) focus on
content, not disclosure. Agent frameworks have no governance layer.

### 1.2 What Must Stay Open

To achieve reference implementation status, the following components must remain
Apache 2.0. Paywalling any of these would fracture the ecosystem and invite
competing forks.

| Component                    | Purpose                                    | Why It Must Be Open                                |
| ---------------------------- | ------------------------------------------ | -------------------------------------------------- |
| `ret-logic`                  | Universal predicate algebra                | The "grammar" of requirements—must be standardized |
| `decision-gate-core`         | Deterministic engine, schemas, runpacks    | Trust requires transparency; auditors need source  |
| `decision-gate-mcp`          | MCP tool surface                           | Protocol must be standard, not proprietary         |
| `decision-gate-providers`    | Built-in providers (time, env, json, http) | Batteries included reduces adoption friction       |
| `decision-gate-broker`       | Reference sources/sinks                    | Reference implementations prevent fragmentation    |
| `decision-gate-provider-sdk` | TypeScript, Python, Go templates           | Ecosystem growth requires accessible tooling       |
| `decision-gate-contract`     | Schema generation, docs                    | Tooling transparency builds trust                  |
| `decision-gate-store-sqlite` | Reference persistence                      | Deployable standalone without vendor lock-in       |
| `decision-gate-cli`          | CLI tooling                                | Complete usability out of the box                  |

### 1.3 Engineering Quality as Moat

The codebase engineering standards ARE the differentiation. Competitors would
need significant time and discipline to replicate:

- **Nation-state threat model**: Assumes hostile inputs at every boundary
- **Fail-closed semantics**: Unknown evidence blocks advancement; explicit routing required
- **RFC 8785 canonical JSON**: Bit-for-bit determinism across environments
- **Zero panics in production code**: Explicit error handling throughout
- **Complete test matrix**: System tests cover P0/P1 scenarios
- **Hyperscaler-grade standards**: "If it wouldn't survive a security review at defense/government, it doesn't ship"

This quality bar cascades to all adopters. Organizations get defense-grade
governance without building it themselves.

### 1.4 The "Reference Implementation" Bar

A reference implementation must be:

1. **Complete**: All documented features work as specified
2. **Correct**: Deterministic, auditable, verifiable
3. **Documented**: Clear onboarding, integration guides, examples
4. **Extensible**: Provider SDK enables ecosystem growth
5. **Trustworthy**: Open source, transparent security model

Decision Gate meets these criteria. The remaining work is documentation polish
and community building—not architectural gaps.

---

## Part 2: Monetization Strategy

### 2.1 Licensing Philosophy

- The open core MUST be fully functional standalone
- Commercial value comes from integration depth, operational excellence, and compliance acceleration
- Never paywall the trust surface (verification, schemas, threat model documentation)

### 2.2 Primary Revenue: Asset Core Integration

Decision Gate + Asset Core = **Auditable decisions within deterministic world state**

Asset Core provides:

- Deterministic state machine (event-sourced, bit-for-bit reproducible)
- Container calculus for 0D–3D+ spaces
- Fixed-point numeric precision (no float drift)
- Chain hashing for tamper-evident history
- Multi-tenant namespace isolation

Decision Gate provides:

- Evidence-backed gate evaluation
- Fail-closed disclosure control
- Cryptographic audit trails
- Offline verification

**Integration Architecture**:

```
AssetCore Namespace
├── Decision Plans (ScenarioSpec instances scoped to namespace)
│   ├── Gates referencing namespace evidence
│   ├── Predicates querying AssetCore containers/capabilities
│   └── Runpacks stored as namespace artifacts
└── Evidence Providers (AssetCore-native)
    ├── Container state queries
    ├── Capability status checks
    ├── Event history predicates
    └── Cryptographic attestations
```

**Value Proposition by Segment**:

| Segment            | Pain Point                      | Decision Gate + Asset Core Value                      |
| ------------------ | ------------------------------- | ----------------------------------------------------- |
| Hyperscalers       | AI safety governance at scale   | Deterministic decisions across multi-tenant workloads |
| Frontier AI Labs   | Model deployment approval gates | Evidence-backed, auditable release decisions          |
| Defense/Government | ITAR/classified data handling   | Fail-closed disclosure with offline verification      |
| Regulated Finance  | Trading system governance       | Replayable decisions for regulatory audit             |
| Healthcare         | PHI disclosure controls         | Policy-gated, evidence-backed data release            |

**Commercial Model**:

- Asset Core SaaS: Decision Gate as namespace-scoped feature
- Asset Core Self-Hosted Enterprise: License includes Decision Gate integration
- Asset Core Defense/Government: Custom deployment + compliance certification

### 2.3 Secondary Revenue: Enterprise Features

Features that warrant commercial licensing or managed service pricing:

| Feature                              | Value Proposition                           | Model                  |
| ------------------------------------ | ------------------------------------------- | ---------------------- |
| **Multi-tenant Runpack Storage**     | Durable, indexed, searchable audit archives | Managed service        |
| **SSO/SAML/OIDC Integration**        | Enterprise identity federation              | Commercial license     |
| **Hardware Security Module Support** | Ed25519 signing via HSM/TPM                 | Commercial license     |
| **FedRAMP/SOC2 Compliance Bundles**  | Pre-validated deployment configs            | License + consulting   |
| **Priority Support SLA**             | Response time guarantees                    | Support contract       |
| **Decision Analytics Dashboard**     | Visualization, metrics, anomaly detection   | Managed service add-on |

### 2.4 Tertiary Revenue: Professional Services

- **Custom Provider Development**: Enterprise-specific evidence adapters
- **Integration Consulting**: Architecture review, deployment planning
- **Compliance Certification Assistance**: Gap analysis, documentation support
- **Training and Onboarding**: Team enablement for Decision Gate adoption

### 2.5 What Must Never Be Paywalled

- Core determinism guarantees
- Runpack verification (trust requires transparency)
- Basic provider development (SDK templates)
- MCP protocol compliance
- Security threat model documentation
- Schema validation tooling

---

## Part 3: Market Segmentation

### 3.1 The Defense-First Strategy

This is counterintuitive but correct:

1. **Defense/government have the hardest requirements**: Nation-state threat models, offline verification, fail-closed semantics, complete audit trails
2. **Building to their standard means everything else is a relaxation**: Healthcare, finance, and general enterprise are strictly easier
3. **Credibility cascades downward**: "If it's good enough for DoD, it's good enough for us"
4. **Marketing writes itself**: "We built for defense. You get defense-grade without the hassle."

### 3.2 Recommended Sequencing

| Phase       | Segment              | Entry Point                              | Revenue Model                    |
| ----------- | -------------------- | ---------------------------------------- | -------------------------------- |
| **First**   | Defense/Government   | Open source adoption + consulting        | Pro services → managed contracts |
| **Second**  | Healthcare/Finance   | Defense credibility + compliance needs   | Enterprise licenses              |
| **Third**   | Hyperscalers/AI Labs | Asset Core integration for AI governance | Platform revenue                 |
| **Ongoing** | General Enterprise   | Community adoption                       | Upsell to managed/support        |

### 3.3 Segment-Specific Value Propositions

**Defense/Government**:

- The engineering standards document speaks their language
- "Nation-state threat model" is actual requirement, not marketing
- Offline verification critical for air-gapped environments
- ITAR/export control use cases align naturally

**Healthcare**:

- PHI disclosure requires evidence-backed decisions
- HIPAA audit requirements match runpack capabilities
- Fail-closed semantics prevent accidental disclosure

**Finance**:

- Trading system governance needs replayable decisions
- SOX/regulatory audit alignment
- Complete decision history for investigation

**AI/ML Teams**:

- Agent guardrails without framework lock-in
- Evidence-backed disclosure for sensitive data
- MCP-native integration with existing tooling

---

## Part 4: Competitive Positioning

### 4.1 Why Decision Gate Is Unique

Decision Gate is the only solution that provides ALL of:

1. **Deterministic, replayable decisions** — Not probabilistic; same inputs → same outputs
2. **Evidence-backed disclosure** — Not trust-based; proof tied to providers
3. **Cryptographic audit trail** — Not log-based; SHA-256 hashes, optional signatures
4. **Fail-closed security** — Not fail-open; unknown blocks advancement
5. **Offline verification** — Not cloud-dependent; runpacks work air-gapped
6. **MCP-native protocol** — Not proprietary API; standard JSON-RPC 2.0
7. **Category-theoretic foundations** — Not ad-hoc logic; universal predicate algebra

### 4.2 Competitor Analysis

| Category                 | Examples                       | Limitation vs Decision Gate                                    |
| ------------------------ | ------------------------------ | -------------------------------------------------------------- |
| **Policy Engines**       | OPA, Cedar                     | No evidence anchoring, no audit trail, no deterministic replay |
| **Workflow Engines**     | Temporal, Airflow              | Not decision-focused, no three-valued logic, no runpacks       |
| **AI Safety Tools**      | Guardrails AI, NeMo Guardrails | Content-focused (not disclosure), not deterministic            |
| **Agent Frameworks**     | LangChain, CrewAI              | No governance layer, no fail-closed semantics                  |
| **Compliance Platforms** | Drata, Vanta                   | Checklist-based, not runtime decision enforcement              |

### 4.3 Competitive Response Strategies

**If OPA/Cedar adopt similar patterns**:

- Response: "We integrate with OPA/Cedar. They handle policy definition; we handle decision execution and audit."
- Position: Decision Gate is the runtime, policy engines are the ruleset.

**If agent frameworks add governance**:

- Response: "We're framework-agnostic. Use Decision Gate with any agent framework."
- Position: Governance layer, not framework lock-in.

**If cloud providers offer similar services**:

- Response: "We're portable. Run anywhere, verify offline, no vendor lock-in."
- Position: Open standard vs. proprietary platform.

### 4.4 Moat Construction

1. **Protocol Moat**: MCP-native, open protocol creates network effects
2. **Trust Moat**: Transparent codebase enables security credibility
3. **Quality Moat**: Engineering standards others can't match quickly
4. **Integration Moat**: Asset Core integration is unique capability
5. **Community Moat**: Provider ecosystem in multiple languages

---

## Part 5: Strategic Phases

### Phase 1: Reference Implementation Credibility

**Objective**: Establish Decision Gate as the standard for deterministic AI decision governance.

**Actions**:

- Publish to crates.io (standalone visibility)
- Create "Getting Started in 5 Minutes" documentation
- Launch reference provider implementations (MongoDB, PostgreSQL)
- Establish GitHub community processes (issues, discussions, contributing)
- Conference presence at AI governance and Rust ecosystem events

**Success Indicators**:

- GitHub stars and community engagement
- First production deployments outside internal use
- Community-contributed providers
- Citations in AI governance discussions

### Phase 2: Commercial Foundation

**Objective**: Validate commercial viability and establish revenue foundation.

**Actions**:

- Asset Core integration design and alpha release
- Defense/government pilot engagements
- First commercial support contracts
- Conference talks (KubeCon, AI governance forums)
- Case studies from early adopters

**Success Indicators**:

- Signed pilot agreements
- First commercial revenue
- Asset Core integration in production use
- Ecosystem of 10+ community providers

### Phase 3: Scale

**Objective**: Mature commercial offering and establish market position.

**Actions**:

- Asset Core managed service GA with Decision Gate
- Enterprise licensing program
- FedRAMP moderate certification
- HIPAA/SOC2 compliance bundles
- Scale community to 50+ providers

**Success Indicators**:

- Recurring commercial revenue
- Multiple enterprise customers
- Compliance certifications achieved
- Recognition as category leader

---

## Part 6: Risk Analysis

### Technical Risks

| Risk                             | Mitigation                                             |
| -------------------------------- | ------------------------------------------------------ |
| Complexity barrier to adoption   | Excellent documentation, examples, SDK templates       |
| Performance at hyperscaler scale | Benchmarking, optimization, horizontal scaling design  |
| Protocol fragmentation           | Strong MCP alignment, reference implementation quality |

### Business Risks

| Risk                           | Mitigation                                                    |
| ------------------------------ | ------------------------------------------------------------- |
| Cloud provider commoditization | Open protocol, offline verification, no lock-in               |
| Slow enterprise adoption       | Defense-first strategy builds credibility cascade             |
| Asset Core coupling concerns   | Decision Gate standalone is complete; integration is additive |

### Community Risks

| Risk                         | Mitigation                                              |
| ---------------------------- | ------------------------------------------------------- |
| Contributor burnout          | Clear scope, automated tooling, maintainable codebase   |
| Fork competition             | Apache 2.0 allows this; compete on quality and velocity |
| Corporate capture perception | Transparent governance, community voice in roadmap      |

---

## Appendix A: Technical Differentiation Summary

The category-theoretic foundations are NOT marketing. They provide:

1. **Morphism-based evaluation**: Requirements are composed algebraically via ret-logic
2. **Three-valued logic**: Unknown is first-class (Kleene/Bochvar modes), not collapsed to false
3. **Canonical serialization**: RFC 8785 ensures bit-for-bit determinism across environments
4. **Container calculus**: Asset Core integration maps naturally to namespace scopes

This is PhD-level computer science implemented to production standards. Competitors
would need significant effort to replicate.

## Appendix B: Glossary

- **RET**: Requirement Evaluation Tree — the universal predicate algebra
- **Runpack**: Cryptographically-verifiable audit bundle for offline replay
- **Evidence**: Provider output with hash, anchor, optional signature
- **Gate**: A requirement tree that must pass to advance a stage
- **Predicate**: A named evidence check bound to a provider query
- **Provider**: An MCP server that supplies evidence for predicates
- **Scenario**: The full definition of stages, gates, and predicates
- **Asset Core**: Deterministic state machine / world substrate (separate product)

## Appendix C: Decision Log

| Decision                                 | Rationale                                                |
| ---------------------------------------- | -------------------------------------------------------- |
| Apache 2.0 for all core components       | Reference implementation requires trust and transparency |
| Asset Core as primary commercial vehicle | Unique integration creates defensible revenue            |
| Defense-first market sequencing          | Hardest requirements establish credibility cascade       |
| MCP-native protocol                      | Alignment with emerging AI tooling standards             |
| No paywall on verification               | Trust surface must remain transparent                    |
