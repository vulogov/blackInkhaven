# Tutorial 53 — Bund script trust gate

*Inkhaven 1.2.15+*

Before 1.2.15, opening any inkhaven project silently
evaluated every Bund script under the `Scripts` system
book.  Open a project shared by someone else and their
scripts ran inside your inkhaven process with no warning.

1.2.15 introduces a trust gate: scripts run only when the
user has affirmatively opted in.  This tutorial explains
the gate, when to enable it, and how to think about the
trust decision.

This is part of the broader security model documented in
[`Documentation/SECURITY_WARNING.md`](../SECURITY_WARNING.md);
read that file first if you're opening a project from a
source you don't know well.

## The three trust decisions

The gate has three settings, controlled by the HJSON field
`scripting.trust_decision`:

| Value | Behaviour |
|-------|-----------|
| `"ask"` (default) | Scripts run only when `<project>/.inkhaven/trust` exists and contains the marker line `trust`. |
| `"trust"` | Scripts run unconditionally. |
| `"deny"` | Scripts never run, regardless of the trust file. |

Default behaviour ("ask" without the trust file) skips
script auto-load and writes a warning to `.inkhaven.log`:

```
N script(s) pending opt-in.  Create `<project>/.inkhaven/
trust` (with the marker line `trust`) to enable, or set
`scripting.trust_decision: "trust"` in inkhaven.hjson if
you authored / audited these scripts.
```

The TUI starts normally without the scripts.  Anything that
depended on them (custom keybindings registered via
`ink.key.bind`, hooks like `hook.on_save`, etc.) will not
fire until you make the trust decision.

## Trusting a project you authored

If you wrote the scripts yourself — or audited every one in
the `Scripts` system book and understand what they do — you
have two options.

### Option A: HJSON declaration

Add to your project's `inkhaven.hjson`:

```hjson
{
  scripting: {
    trust_decision: "trust"
  }
}
```

This is convenient because the decision rides along with
the project — if you `git clone` your manuscript on a new
machine, the scripts work immediately.

**Trade-off:** the HJSON is part of the project files.  If
you share the project later (a `.zip`, a public repo), the
recipient inherits your trust declaration.  They should
either remove or audit it before opening.

### Option B: Trust file marker

```bash
$ mkdir -p .inkhaven
$ echo trust > .inkhaven/trust
```

The marker file is local to your checkout and conventionally
gitignored (the `.inkhaven/` directory holds rescue files,
recovered crash reports, health/doctor logs — all
machine-local state).  Recipients of a shared project never
inherit your trust file.

This is the recommended pattern when you want to grant
trust to your own copy but still ship a "safe by default"
project to readers.

## Opening someone else's project

When you receive a project from an external source, leave
`trust_decision` at its default (`"ask"`) and do not create
the trust file until you have read what the scripts do.

The scripts live as paragraphs under the `Scripts` system
book in the project tree.  Open them in the editor like any
other paragraph; they're plain Bund source code.

Things to look for when auditing a script:

* **`ink.fs.*` calls.**  In 1.2.15 these are confined to
  the project root by default (S.6.H2 sandbox), but a
  script that asks for `scripting.fs_unsandboxed: true` is
  asking for access to anywhere your user account can
  reach.
* **`ink.input` calls with credential-shaped prompts.**  A
  script can pop a modal that says "Enter your token:" —
  there is no built-in way for you to know the prompt is
  script-driven vs. inkhaven-driven (pending S.6.M3
  fix).
* **`ink.key.bind` / `ink.key.bind_lambda` calls.**  These
  rebind chords.  A script that rebinds `Ctrl+S` to a
  custom lambda may or may not preserve save semantics.
* **`hook.on_save` / `hook.on_diagnostic` / `hook.on_*`
  registrations.**  These fire silently on every relevant
  event.  Read what they do.
* **Calls to enabled-category Bund stdlib words.**  Words
  like `ink.tree.delete`, `ink.paragraph.save`,
  `ink.ai.send`, `ink.editor.replace_all` are all gated
  default-deny.  If the project's HJSON enables those
  categories, scripts can use them.

If anything looks suspicious, leave the trust gate engaged.
The TUI works without the scripts; they're an enhancement,
not a load-bearing component.

## Reviewing a project for read-only inspection

If you want to open a project just to read its prose —
without running any of its scripts ever — use:

```hjson
{
  scripting: {
    trust_decision: "deny"
  }
}
```

This blocks script auto-load regardless of what the
`.inkhaven/trust` file says.  Useful for opening a colleague's
manuscript to comment on chapters without inheriting their
build-automation hooks.

## Interaction with the existing policy system

The trust gate is layered on top of the existing scripting
policy.  Even when `trust_decision: "trust"` runs the
scripts, those scripts can only call the `ink.*` stdlib
words that the policy allows:

* `Policy::default()` denies `STORE_WRITE`, `EDITOR_WRITE`,
  `AI_WRITE`, `THEME_WRITE`, `FS_WRITE`, `NET`, `SHELL`,
  `CODE_EVAL`, and `KEYMAP`.
* `FS_READ`, `STORE_READ`, `EDITOR_READ`, `AI_READ`, and
  `AUDIO` are allowed by default.

A trusted-but-conservatively-policied project gives the
script the read surface but blocks destructive calls.  You
can opt in to specific categories via
`scripting.enabled_categories: ["fs_write"]` in HJSON; do
this only on projects you trust enough to grant write
access.

The 1.2.15 audit notes that even default categories provide
enough surface for a malicious script to exfiltrate project
content (via `ink.fs.read` to anywhere readable) or display
phishing modals (via `ink.input`).  The trust gate is the
strongest mitigation: refuse to run scripts you didn't
write.

## See also

* [Tutorial 51 — Crash report writer and recover](51-crash-report-and-recover.md)
* [Tutorial 52 — Health monitor and doctor scan](52-health-and-doctor.md)
* `Documentation/SECURITY_WARNING.md` — risk disclosure
* `Documentation/RELEASE_NOTES/1.2.15.md` — Phase S.6
  (security hardening)
* `src/scripting/policy.rs` — Policy struct + category
  taxonomy
