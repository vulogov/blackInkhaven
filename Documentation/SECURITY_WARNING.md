# Inkhaven — Security Warning and Disclaimer

> **Read this before opening a project file you did not author.**
> Inkhaven is provided **"AS IS"** with no warranty of any kind.
> By installing, running, or using Inkhaven, you accept the
> risks described below.  If you do not accept them, do not
> use this software.

This document is current as of Inkhaven version 1.2.15.  It may
not enumerate every issue that exists; see §5 ("Unknown Risks")
and §7 ("Reporting Security Issues").

---

## 1. License and "No Warranty" Notice

Inkhaven is distributed under the terms of the Apache License,
Version 2.0 AND the MIT License (dual-licensed; see the
[`LICENSE`](../LICENSE) file in the repository root).  Both
licenses contain a standard *no-warranty* clause; the substance
is reproduced here in plain English for readers who do not
routinely parse legal text:

> **THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
> KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
> WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
> PURPOSE AND NONINFRINGEMENT.**

You receive Inkhaven free of charge and use it voluntarily.
The author has not been paid by you to deliver fitness for any
particular purpose, and has made no representation that the
software is free of defects, secure against any specific
attack, or suitable for any specific environment.  Your use of
Inkhaven is **at your own risk**.

## 2. Limitation of Liability

To the maximum extent permitted by applicable law:

**The author of Inkhaven shall not be liable to you or to any
third party for any direct, indirect, incidental, special,
consequential, punitive, or exemplary damages of any kind,
including but not limited to:**

* **Personal damages** — including but not limited to loss of
  productivity, loss of unsaved work, frustration, time spent
  recovering from a defect, or any other personal harm arising
  from the use or inability to use Inkhaven.
* **Business damages** — including but not limited to loss of
  revenue, loss of contracts, loss of goodwill, loss of
  reputation, business interruption, or any consequential
  loss to a business or organisation that uses Inkhaven or
  relies on its output.
* **Financial damages** — including but not limited to direct
  monetary loss, fines, regulatory penalties, costs of
  remediation, costs of substitute services, costs of recovery
  from data loss, or any other financial harm.

This limitation applies whether the claim is in contract, tort
(including negligence), strict liability, breach of statutory
duty, or any other legal theory, even if the author has been
advised of the possibility of such damages and even if a
limited remedy elsewhere fails of its essential purpose.

**If you cannot accept this limitation, do not use Inkhaven.**

## 3. User Responsibilities

By using Inkhaven you accept the following responsibilities:

3.1 **Back up your work.**  Inkhaven includes a backup
subsystem (`inkhaven backup`, `Ctrl+B Shift+B`, exit-hook
auto-backup) but it cannot recover work that was never saved
or backed up.  Maintain independent copies of important
manuscripts.

3.2 **Audit untrusted projects before opening them.**  An
Inkhaven project is a directory of `.typ` files plus an
`inkhaven.hjson` config plus an optional `Scripts` system
book containing arbitrary Bund script source.  Opening a
project from an untrusted source (a shared `.zip`, a public
repository you have not reviewed, a colleague you do not
know well) may grant that project the ability to:

  * **Run code in the Bund scripting VM at project open.**
    1.2.15+ requires the file `<project>/.inkhaven/trust`
    (containing a `trust` marker line) OR the HJSON setting
    `scripting.trust_decision: "trust"` before any script
    auto-loads.  Do not create the trust file or set the
    HJSON value unless you have read every paragraph under
    the `Scripts` system book and understand what it does.

  * **Read or write files inside the project tree** via the
    `ink.fs.read` / `ink.fs.write` Bund words, even when the
    sandbox is in default mode.  Without the sandbox
    (`scripting.fs_unsandboxed: true`) those words can read
    or write anywhere your user account can reach.  Do not
    set `fs_unsandboxed: true` on a project you did not
    author.

  * **Display arbitrary text in input modals.**  A Bund
    script can pop a modal with any prompt string.  A
    malicious project might pop "Enter your GitHub token:"
    to capture credentials.  This is a known UX risk; see
    §4.2 (M3) below.

3.3 **Treat HJSON config fields as untrusted input** if the
config came from an untrusted source.  Inkhaven 1.2.15
filters `..` traversal on `prompts_file` and
`artefacts_directory`, but absolute paths are honoured per
documented intent — opening a malicious config could still
cause Inkhaven to read or write outside the project tree.

3.4 **Keep your LLM API keys in environment variables, never
in committed config.**  Inkhaven reads keys only from env
vars by design.  Pasting keys into the AI prompt pane risks
their inclusion in a crash report (see §4.1 below).

3.5 **Review crash reports before sharing them.**  The
1.2.15 crash report writer (`inkhaven-crash-<ts>.hjson`)
deliberately omits LLM prompts, search queries, snapshot
bodies, and most environment variables — but the
*recent-action ring* may contain a small amount of
context-derived information.  If a crash occurred while you
had a secret on the clipboard or in the AI input, that
secret may be partially captured in the action ring's
`detail` field.  Read the file before posting it in a bug
report.

## 4. Known Security Issues (1.2.15)

The following issues are catalogued in the 1.2.15 Security
Audit.  All are documented here so users can make informed
decisions; some are fixed in code, some are pending, some are
inherent properties of the design.

### 4.1 Fixed in 1.2.15

These were identified during the audit and patched before the
1.2.15 release:

* **H1 — Bund script auto-load without consent (FIXED).**
  Previously, opening any project silently evaluated every
  Bund script under the `Scripts` system book.  Now gated by
  `scripting.trust_decision` HJSON + the
  `<project>/.inkhaven/trust` file marker.

* **H2 — `ink.fs.*` sandbox bypass (FIXED).**  Previously,
  `ink.fs.read` and `ink.fs.write` accepted any absolute or
  relative path and operated with the user's full filesystem
  permissions.  Now confined to the project root by default
  via `crate::path_safety`.  Opt-out:
  `scripting.fs_unsandboxed: true`.

* **H3 — Path traversal in `inkhaven recover` (FIXED).**
  Previously, the `paragraph_rel_path` field from a crash
  report was joined onto the project root with no `..`
  validation.  A crafted crash report with
  `paragraph_rel_path: "../../etc/passwd"` could have
  written to that path when the user ran `inkhaven recover
  --yes`.  Now validated via `path_safety::resolve_within_str`.

* **H4 — Path traversal in HJSON config (FIXED).**
  Previously, `prompts_file` and `artefacts_directory` from
  `inkhaven.hjson` were joined onto the project root with no
  `..` validation.  Now validated via
  `path_safety::resolve_within_or_absolute`.

### 4.2 Pending — Not Fixed at Time of Writing

* **M1 — Crash-report action ring may capture pasted
  secrets.**  The recent-action ring (`recent_actions` field
  in `inkhaven-crash-<ts>.hjson`) is a 50-entry FIFO of user
  actions.  Each entry's `detail` field is a short
  free-form string.  If a user pastes a secret (API key,
  password) into the AI prompt pane or editor and a panic
  occurs immediately after, that secret may be partially
  visible in the action ring.  Mitigation: §3.5 above (review
  before sharing).  Planned remediation: audit all
  `push_action` call sites; truncate `detail` to ~80
  characters at insertion.

* **M3 — Script-initiated input modals can mimic legitimate
  prompts.**  A Bund script with the appropriate categories
  can pop `ink.input` modals with arbitrary prompt strings.
  A malicious project could pop "Enter your password:" and
  exfiltrate the response.  Default sandbox limits where the
  exfiltration can write to, but the *display* surface
  remains.  Mitigation: §3.2 above (do not open untrusted
  projects).  Planned remediation: prefix script-initiated
  modals with a visible `[script: <slug>]` tag so users can
  distinguish them from Inkhaven-internal prompts.

### 4.3 Design Properties (Inherent, Not Bugs)

Some characteristics are inherent properties of the design
and will not be "fixed" in the conventional sense.  Users
should be aware of them when deciding whether to use
Inkhaven for a given purpose.

* **Inkhaven runs as the invoking user.**  It has no
  privilege separation, no setuid component, no sandbox of
  its own.  Every operation (file I/O, network calls to LLM
  providers, subprocess invocations of `typst` / `say`)
  inherits the user's permissions.  A compromised Inkhaven
  process is a compromised user account.

* **LLM API calls send data to third parties.**  When a
  non-Ollama provider is configured (Gemini, Claude, OpenAI,
  DeepSeek, Grok), the contents of every prompt, scope
  selection, and inferred response travel to that provider's
  servers under that provider's terms of service and privacy
  policy.  Inkhaven does not implement any privacy
  guarantees on this path.  See the
  [`Documentation/CONFIGURATION.md`](CONFIGURATION.md)
  `llm.default_provider` field — set it to `ollama` and run a
  local model if you require on-device-only operation.

* **Bund scripting can break the editor's invariants.**
  Even with sandbox + trust gate + default-deny categories
  active, a user who explicitly enables `STORE_WRITE` or
  `EDITOR_WRITE` is granting scripts the ability to mutate
  their project state.  Inkhaven cannot reason about whether
  a user-authored script will preserve data integrity; the
  default policy denies destructive categories precisely
  because the trust assumption is hard.

* **TLS validation is delegated to dependency crates.**
  Inkhaven uses `genai` for LLM HTTP and `fastembed` for
  ONNX model download.  Both rely on the platform's TLS
  stack via `reqwest` / `hyper`.  Inkhaven enables no
  certificate-validation bypass flags, but the underlying
  trust is the same as any other HTTPS client running on
  your machine.

## 5. Unknown Risks

The 1.2.15 audit was thorough but not exhaustive.  The
following classes of risk exist by virtue of the software
being complex code written by humans:

5.1 **Undiscovered vulnerabilities may exist.**  The audit
examined catalogued attack surfaces (subprocess calls, path
handling, secrets, Bund sandbox, TLS, dependency surface).  It
did not, and could not, prove the absence of bugs.  Use
appropriate caution for the value of the data you entrust to
Inkhaven.

5.2 **Dependencies may have vulnerabilities.**  Inkhaven
depends on dozens of third-party crates (ratatui, crossterm,
genai, fastembed, bdslib, tree-sitter-typst, serde, tokio,
and many transitive dependencies).  Each is independently
maintained; new vulnerabilities are discovered regularly
across the Rust ecosystem.  Inkhaven's release cadence
updates dependencies periodically but not instantly.
Running `cargo audit` on your install gives you a current
view; the author makes no guarantee about the cadence of
dependency security updates.

5.3 **The Bund scripting surface is evolving.**  New `ink.*`
stdlib words and new policy categories are added in most
releases.  A category that is safe today may interact in
unforeseen ways with a future word.  Users who run
non-default policy configurations should re-review their
policy after each Inkhaven release.

5.4 **Platform-specific behaviour may differ.**  Path
validation, TLS configuration, and subprocess semantics
differ between macOS, Linux, and Windows.  The audit was
performed primarily against macOS behaviour; Linux-specific
or Windows-specific edge cases may exist.

5.5 **Documentation may be wrong.**  This file describes the
state of the software as of 1.2.15.  Subsequent commits may
change behaviour; the documentation cannot be assumed
synchronized with the code in real time.  Code is
authoritative.

## 6. No Indemnification

Inkhaven does not, and the author does not, indemnify you
against any claim, demand, suit, or action by any third party
arising from your use of Inkhaven, including but not limited
to claims arising from:

* prose, prompts, or other content you generate using
  Inkhaven (including but not limited to claims of copyright
  infringement, defamation, or breach of confidentiality);
* failure to comply with the terms of service of any LLM
  provider you configure;
* damage to systems, data, or persons caused by Bund
  scripts you author or run.

## 7. Reporting Security Issues

If you discover a security issue in Inkhaven that is not
listed in §4 above, please report it via the project's
[GitHub Issues page](https://github.com/vulogov/blackInkhaven/issues)
with the prefix `[SECURITY]` in the title.  For issues you
believe should be discussed privately before public
disclosure, contact the author through the email address
listed in the repository's `Cargo.toml` `authors` field
before filing a public issue.

The author makes no commitment to a specific response time,
patch timeline, or coordinated disclosure procedure.
Reports are reviewed on a best-effort basis.

## 8. Acceptance of These Terms

Your continued use of Inkhaven constitutes your acceptance of
the terms of this document, the Apache 2.0 License, and the
MIT License.  If at any point you cease to accept these
terms, you must stop using Inkhaven and remove it from your
systems.

---

*This document is part of the Inkhaven project documentation
and is subject to the same license as the rest of the
repository (Apache-2.0 AND MIT).  It does not, and cannot,
override the terms of the licenses themselves; in any
conflict, the licenses control.*

*Last updated: with Inkhaven 1.2.15-dev.  See the project
release notes and git history for changes.*
