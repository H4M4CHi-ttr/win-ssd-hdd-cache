# F02 planning checker contract v1

Additive implementation details for [contracts §§5/11](../contracts.md). This checker validates a reviewed planning snapshot; it does not fetch GitHub, approve an owner decision, prove linked evidence, or change any task's state. Existing schema-v2 gates remain unchanged.

## APIs, input and diagnostics

The safe-Rust `xtask` library exposes:

```rust
pub struct ManifestViolation { pub code: String, pub path: String }
pub fn validate_label_manifest(input: &str) -> Result<(), Vec<ManifestViolation>>;
pub fn validate_backlog(input: &str, known_labels: &BTreeSet<String>)
    -> Result<(), Vec<ManifestViolation>>;
pub struct PlanningSummary {
    pub labels: usize, pub tasks: usize, pub documents: usize, pub relative_links: usize,
}
pub struct RepositoryViolation { pub file: String, pub code: String, pub path: String }
pub fn check_repository(root: &Path)
    -> Result<PlanningSummary, Vec<RepositoryViolation>>;
pub fn validate_markdown_links(root: &Path, document: &Path, source: &str)
    -> Result<usize, Vec<ManifestViolation>>;
```

Types derive Debug/Clone/PartialEq/Eq; violations additionally Ord/PartialOrd in declared field order. Arrays of violations are sorted and deduplicated. API consumers supply an already validated label-name set to `validate_backlog`; `check_repository` enforces that ordering itself.

- Maximum JSON input: labels 1 MiB, backlog 4 MiB; container nesting at most 64. At most 256 labels and 1,024 tasks; the completed baseline requires at least one task. These are checker-resource bounds, not product resource budgets.
- Every required key is present. Objects, arrays, strings, booleans and unsigned integer tokens are distinct. Unknown/duplicate keys, including escaped equivalents, are rejected. Schema integer tokens are not float/exponent/string/negative tokens.
- Parse enough raw structure to retain duplicate keys and numeric token spelling; never erase duplicates through a normal Value map or infer paths from raw parser-error strings.
- Syntax, trailing document, byte/depth/collection bounds: `INVALID_JSON`, path `""`. Wrong type, missing required field and duplicate key: `INVALID_JSON` at that field; an unknown key: `UNKNOWN_FIELD` at that field. JSON Pointer escapes `~` and `/` normally. A structural error may stop at one failure.
- Well-typed but unsupported schema integer: `UNSUPPORTED_SCHEMA` at `/schema_version`.
- A fully structurally parsed manifest/backlog aggregates semantic violations; do not mutate input to make it valid.

## Label semantics

Keep exactly the seven codes and existing grammar from contracts §5. Name equality is case-sensitive; every repeated occurrence after the first produces `DUPLICATE_NAME` at `/labels/i/name`. Name length is bytes and description length is Unicode scalar values. Do not add an undocumented lowercase/nonempty-suffix restriction. Collection bounds are checked before label traversal.

## Backlog semantics

In addition to structural codes, use:

`INVALID_FIELD`, `DUPLICATE_ID`, `DUPLICATE_ISSUE`, `UNKNOWN_LABEL`, `DUPLICATE_DEPENDENCY`, `MISSING_DEPENDENCY`, `SELF_DEPENDENCY`, `DEPENDENCY_CYCLE`, `INVALID_COMPLETION`, `INVALID_APPROVAL`, `UNREADY_DEPENDENCY`, `OWNER_APPROVAL_REQUIRED`.

- IDs are 1–32 uppercase ASCII letters/digits. Title/output/escalation and each acceptance string must be nonempty after trimming. Files/dependencies/acceptance/evidence must be arrays of strings; acceptance and files must be nonempty. Planned file paths must be nonempty, relative, use `/`, and contain no parent traversal, drive colon or NUL; they need not exist.
- plan_date is a real Gregorian YYYY-MM-DD, years 0001–9999, no current-clock dependency.
- Require the enum values in contracts §11 and their corresponding manifest labels; manifest presence alone does not expand an enum (for example type:tracking remains invalid for a work task). Area must resolve to an area label. Owner-required tasks also require decision:owner.
- issue_number is a positive unsigned JSON integer and unique. issue_url exactly equals this repository's HTTPS issue URL with that decimal number, without query/fragment/trailing path. Null issue metadata is not accepted in this completed-baseline checker.
- Preserve done iff accepted, cancelled iff not-planned, and pending for all other states. Report `INVALID_COMPLETION` at the task's `/completion`.
- Preserve owner_decision/owner_approval/evidence consistency. Report `INVALID_APPROVAL` at `/owner_approval` for an inconsistent combination, or at a malformed `/approval_evidence/i` URL. Approved evidence accepts only this repository's `issues/{positive integer}` or `pull/{positive integer}`, optionally with a nonempty fragment; no queries or other paths. Pending plus an evidence URL never becomes approved. A reviewer must still inspect actual authority and scope.
- ready or accepted requires all dependency completions accepted and owner approval not-required or approved. Report unaccepted dependencies at `/dependencies/i`, owner gating at `/owner_approval`. needs-decision requires owner-required + pending; blocked may remain valid even if its listed dependencies are accepted, because contracts may still be unresolved.
- Duplicate IDs make reference resolution ambiguous: report them and skip dependency resolution, graph and dependency-readiness checks. Invalid IDs likewise skip resolution. Independent field, owner-approval and completion validation still run. Otherwise check dependencies, using an iterative graph algorithm; report one `DEPENDENCY_CYCLE` at `/tasks` if any cycle exists. Report individual missing/self/duplicate dependency violations too.
- Invalid field/enum errors point at that field. Duplicate IDs/issues point at later occurrences. Do not weaken owner or safety gates to make baseline validation pass.

## Relative Markdown links and repository execution

Use a pinned CommonMark parser's Link/Image events, not a regex. Scan root `.md` files and `.md` files recursively under `docs`, `.github`, and `examples`; skip missing optional directories, do not follow directory/file symlinks, and reject a symlink at the required JSON inputs. Limit document traversal to depth 32, 2,048 documents, and 2 MiB UTF-8 bytes per document. A checkout must contain at least one document. I/O or traversal failures are fixed `INPUT_IO`; resource failures are `RESOURCE_LIMIT`.

- Ignore external scheme URLs, protocol-relative URLs, fragment-only and empty destinations; do not request any remote resource. Reject absolute local/Windows-drive paths.
- For relative destinations, strip query/fragment, percent-decode `%HH` (not `+`), require UTF-8 and no NUL/backslash, resolve from the document's parent, and require an existing file or directory whose canonical path remains inside the canonical repository root. Check absolute/Windows-drive paths before external-scheme classification and again after decoding; an encoded local path must not evade the check. Decoded #/? are filename characters because stripping precedes decoding. Missing/escaping/invalid destinations yield `INVALID_LINK` at synthetic `/links/i`, where i counts Link/Image events in source order (including skipped external events).
- Escaped/reference/image links follow parser behavior. Raw HTML, heading-anchor correctness and unspecified undefined reference labels are not checked. Inline/fenced code paths and backlog `files` are not links and do not need to exist.
- Existing repository files are trusted/cooperatively stable while checking. This unprivileged tool is not an adversarial filesystem sandbox or privileged target-authorization API.

`cargo xtask check-planning` uses the repository root derived from `CARGO_MANIFEST_DIR`, not the current working directory. Exactly that subcommand and no extra arguments are accepted. The executable performs no network, credential, Git, automatic-edit, label, or deletion operations. Cargo itself may write normal build artifacts; the validator does not modify the repository.

Success: exit 0 and one stdout line `planning: ok (labels=N, tasks=N, documents=N, relative_links=N)\n`, stderr empty. Failure: exit 1, stdout empty; each sorted violation as one JSON stderr line with keys `file`, `code`, `path` (proper JSON escaping, no raw parser message). Bad CLI arguments: exit 2, stderr exactly `error: USAGE\n`. Full Markdown table/prose equivalence with JSON is a publication-review responsibility, not silently added as a parser acceptance rule.
