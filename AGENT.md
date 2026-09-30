# AGENT.md

## Project Principles

Build this project as a clean, lean, high-performance open-source Rust application.

The codebase starts blank. Do not force architecture, libraries, patterns, file layouts, abstractions, or implementation details before they are justified by real requirements. Prefer simple foundations that remain easy to extend.

## Working Rules

- Make surgical changes. Touch only the code required for the current task.
- Preserve existing behavior unless the task explicitly requires changing it.
- Keep diffs small, focused, reviewable, and easy to revert.
- Prefer clear, idiomatic Rust over clever code.
- Avoid premature abstraction, speculative features, unnecessary indirection, and over-engineering.
- Reuse existing concepts before introducing new ones.
- Keep modules cohesive and responsibilities narrow.
- Separate reusable concerns from feature-specific implementation when that separation is genuinely useful.
- Design extension points so new file formats or features can be added without spreading format-specific logic across unrelated parts of the application.
- Keep UI, application logic, editing logic, parsing/formatting concerns, and infrastructure loosely coupled where practical.
- Avoid global branching on concrete formats when a registry, capability, or local abstraction would scale better.
- Prefer composition and small interfaces over large inheritance-like abstractions.
- Do not create a crate, module, trait, service, or abstraction unless it earns its complexity.

## Performance

Performance is a first-class requirement from the beginning.

- Optimize for fast startup, low idle CPU usage, low memory usage, and responsive interaction.
- Avoid unnecessary allocations, copies, clones, temporary buffers, and repeated parsing.
- Avoid loading or initializing work that is not needed yet.
- Prefer lazy initialization when it reduces startup cost without making the design fragile.
- Keep hot paths small and predictable.
- Be conscious of ownership, allocation behavior, data layout, caching, and synchronization costs.
- Do not add background work, threads, async machinery, polling, or caches without a concrete need.
- Measure before introducing complex micro-optimizations, but do not knowingly build expensive foundations that will need to be replaced later.
- Prefer efficient defaults over relying on future optimization passes.

## Memory

Memory efficiency matters.

- Keep long-lived state minimal.
- Avoid duplicating document contents or large derived structures without a strong reason.
- Release temporary data promptly.
- Bound caches and history where appropriate.
- Avoid retaining UI state, parsed representations, diagnostics, or formatted output longer than necessary.
- Prefer incremental or on-demand work when it materially reduces memory pressure.

## Safety and Reliability

- Write safe Rust by default.
- Avoid `unsafe` unless there is a demonstrated need, measurable benefit, and a clearly documented safety invariant.
- Treat external input, file contents, paths, encodings, and parser failures as untrusted.
- Avoid panics in normal application flows.
- Return meaningful errors and diagnostics at appropriate boundaries.
- Keep failure contained; one malformed file should not destabilize the application.
- Do not silently ignore errors that can affect correctness.
- Never weaken security or correctness for convenience.

## Dependencies

- Keep the dependency tree small.
- Add a dependency only when it provides clear value over a small, maintainable internal implementation.
- Prefer mature, focused, well-maintained crates.
- Avoid large frameworks for small needs.
- Disable unnecessary crate features.
- Be mindful of compile time, binary size, startup cost, transitive dependencies, and runtime overhead.
- Do not introduce multiple libraries solving the same problem without a strong reason.

## Architecture

Architecture should emerge from requirements while preserving clear boundaries.

- Keep core logic independent from UI-specific code where practical.
- Keep format-specific behavior local to the format implementation.
- Prefer capability-based design for parsing, formatting, validation, highlighting, detection, serialization, or similar concerns when needed.
- New formats should be addable with minimal changes outside their own implementation.
- Avoid central files that must be edited for every small feature unless they serve a clear registry or composition purpose.
- Keep domain types small and purpose-driven.
- Avoid leaking third-party library types throughout the codebase when doing so would create unnecessary coupling.
- Do not introduce plugin systems, dynamic loading, generic frameworks, or complex registries before the project actually needs them.

## Testing

- Keep tests separate and organized by purpose.
- Test behavior, contracts, edge cases, and failure modes rather than implementation details.
- Prefer small unit tests for local logic and focused integration tests for boundaries.
- Use fixtures for representative file-format cases when useful.
- Shared behavior across formats should use reusable contract-style tests when appropriate.
- Every bug fix should include a regression test when practical.
- Tests must remain deterministic, fast, and easy to understand.

## Code Quality

- Run formatting and relevant checks before considering a task complete.
- Keep warnings clean.
- Use descriptive names and small functions.
- Comments should explain non-obvious intent, invariants, tradeoffs, or constraints—not restate the code.
- Remove dead code, abandoned experiments, debug output, and temporary workarounds.
- Avoid TODOs unless they capture a real deferred decision with enough context to be actionable.
- Public APIs should be deliberate and minimal.
- Internal APIs should remain easy to change.

## Change Discipline

Before changing code:

1. Understand the existing flow and the smallest correct change.
2. Check whether the required behavior already exists.
3. Avoid unrelated cleanup unless it is necessary for the task.
4. Preserve architectural boundaries.
5. Consider startup time, runtime cost, memory usage, safety, and maintainability.

After changing code:

1. Review the diff for unnecessary changes.
2. Remove accidental complexity.
3. Run relevant tests, formatting, linting, and checks.
4. Confirm no unrelated behavior changed.
5. Keep the final implementation smaller and clearer whenever possible.

## Decision Rule

When several solutions are valid, prefer the one that is:

- simpler,
- easier to reason about,
- easier to test,
- easier to remove or replace,
- lower in runtime and memory overhead,
- less coupled,
- safer,
- and more maintainable for outside contributors.

Do not optimize for hypothetical future requirements at the cost of present clarity. Build clean extension points, not speculative systems.
