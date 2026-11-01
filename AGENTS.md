# Agent instructions

Read README.md, docs/README.md, docs/agent-routing.md, and the assigned issue before changing code.

## Non-negotiable
- Primary implementation: Rust. Necessary GUI: Tauri (Vue/TypeScript frontend). Use C/WDK or another SDK only at justified Windows/OCF boundaries with an ADR.
- This repository is design-only at bootstrap. Do not claim a production driver, verified Windows support, or completed hardware tests.
- HDD remains the only authoritative copy. Initial product is clean read cache / Write-Invalidate, not write-back. No persistent cache reuse.
- Follow INV-01 through INV-12 in the design. A read cache can still corrupt the origin indirectly by returning stale filesystem metadata.
- Never change a failing test's expected data just to match the implementation.
- No raw-disk write, formatting, driver installation, boot-setting change, Verifier, hardware fault injection, certificate purchase or public release without separate explicit lab/owner authorization.
- Do not disable Secure Boot/HVCI as the normal product path.
- Pure host crates forbid unsafe. Kernel/FFI unsafe requires a documented ownership/IRQL/aliasing contract and independent review.
- Pin/Arc/Drop do not prove DMA or device I/O completion.
- Do not implement blocked or decision-pending issues until the required contracts and evidence exist.
- Follow issue ownership/review labels. Escalate undefined safety semantics; do not improvise them.
- Preserve unrelated work. Never force-push or remove user labels automatically.

## Work unit and validation
- One bounded issue per PR; record dependencies and exact commands/results.
- Red test -> minimal implementation -> passing test -> independent review.
- Cargo workspace/shared manifests are integration points: avoid concurrent conflicting edits.
- No network, Windows API, privileged operations or real device access in the pure model/test tools.
- Keep host model, candidate engine and actual Windows qualification evidence distinct.
- Before merging, update docs/contracts and affected issues when a reviewed contract changes.
- No build/test command is considered successful when it was not run or when it ran zero applicable tests.

## Policy decisions
License, repository visibility, signing identity/costs and human release approval belong to the owner. See GOV01/D08. Do not silently choose them on the owner's behalf.
