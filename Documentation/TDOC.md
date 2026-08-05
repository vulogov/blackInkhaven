# Technical-Documentation Checks (TDOC)

*(1.6, TDOC roadmap — see [`PROPOSALS/TDOC_ROADMAP.md`](PROPOSALS/TDOC_ROADMAP.md),
[`PROPOSALS/TDOC-1_PLAN.md`](PROPOSALS/TDOC-1_PLAN.md))*

Fiction's enemy is self-contradiction; technical documentation's enemy is
**staleness** — prose that was true of the last release and is now a quiet lie about
this one. Inkhaven's soul is checking prose against a ground truth. TDOC turns that on
the documentation author's own ground truth: it makes the docs answer to the **code**
they show, the **cross-references** they promise, and the **release** they ship
against.

> **TDOC is deterministic and advisory.** `docs verify` runs only the commands your
> own config declares, only for code blocks you explicitly marked, and only after an
> explicit `--yes`. `docs links` and `docs review` never touch prose at all. There is
> zero AI and (except the opt-in external link check) zero network anywhere in the
> subsystem.

All three subcommands scan your **user books** only (system/companion books are
skipped) and **exit non-zero** when they find a problem — so each drops straight into a
pre-release or CI gate.

---

## `docs verify` — the code examples still run

A fenced code listing in a `para:code` paragraph can be marked for verification by
adding `verify` to its fence info string — ` ```rust verify `. `docs verify` pulls
every such block out, writes it to a temp file, and runs it through the runner your
project configures for that language. A block that exits non-zero — a stale example
that no longer compiles against the current toolchain — is reported with the runner's
output tail; passing blocks are quiet. `no-verify` in the fence always wins over
`verify`.

```
inkhaven docs verify                       # every user book (nothing runs yet)
inkhaven docs verify --dry-run             # list each block + its resolved command
inkhaven docs verify --yes                 # actually execute the runners
inkhaven docs verify --book-name X         # restrict to one book (slug or title)
inkhaven docs verify --paragraph <slug>    # restrict to one paragraph by slug-path
```

Two safety gates stand between a fresh clone and running code:

- **`docs.verify.enabled` must be `true`** (it is `false` by default). Off ⇒ the
  command refuses and points you at the config.
- **`--yes` is required to execute.** Without it, `docs verify` lists the blocks and
  the runner commands that *would* run with your privileges, then stops. `--dry-run`
  previews the resolved commands (with `{file}` shown) without touching the config
  gate — the safe way to inspect what a project would do.

Each block resolves to one of four outcomes: **pass** (runner exited 0), **fail**
(non-zero exit *or* timed out — carries the last 40 lines of combined output),
**skip** (no runner configured for that language — not an error), or **errored**
(couldn't write the temp file or spawn the runner — counted as a failure). The command
exits non-zero if any block failed.

---

## `docs links` — the cross-references still resolve

```
inkhaven docs links                    # internal cross-references, project-wide
inkhaven docs links --external         # also check http(s) URLs for link-rot (network)
inkhaven docs links --book-name X      # restrict the external sweep to one book
```

- **Internal** (always, deterministic, project-wide over all user books): every
  paragraph's `linked_paragraphs` cross-reference whose target no longer exists in the
  hierarchy — a link to a renamed or deleted node.
- **External** (opt-in, `--external`, network): every `http(s)` URL embedded in prose
  (both bare and `#link("…")`), deduplicated and reported once per URL, checked with
  the same conservative classifier the research `/deadsources` sweep uses — only
  `404`/`410` and hard failures count as dead, so a slow or auth-walled host is not a
  false positive.

Exits non-zero when any link — internal or external — is broken.

---

## `docs review` — the manuscript is current

A currency dashboard over the readiness ladder
(`none` → `napkin` → `first` → `second` → `third` → `final` → `ready`). Per chapter it
prints the status breakdown, then lists every paragraph still below a floor.

```
inkhaven docs review                       # breakdown; below `ready` (the default floor)
inkhaven docs review --floor final         # measure against a lower bar
inkhaven docs review --since v1.6.10        # flag paragraphs changed since a git ref
inkhaven docs review --book-name X         # one book
```

- **`--floor`** — the readiness bar: `napkin` | `first` | `second` | `third` |
  `final` | `ready` (default `ready`). Paragraphs below it are listed as *needs work*.
- **`--since <ref>`** — marks every paragraph whose `.typ` file changed since a git tag
  or commit — the "what do I need to re-read since the last release" view. If the
  project is not a git repo or the ref is unknown, change detection is skipped with a
  note (the rest of the report still runs).

Exits non-zero when any paragraph sits below the floor — the "no half-baked section
ships" gate.

---

## In the editor

**`Ctrl+B Shift+D`** verifies the **open** paragraph. If it is a `para:code` listing,
every `verify`-marked block in it is run synchronously (one listing is quick) through
the same configured runners; failures land in the **Output pane** anchored on the
paragraph (colouring its tree badge and answering the `t` / this-paragraph filter),
and the status line reports `passed · failed · skipped`. Passing blocks are quiet.
Gated on `docs.verify.enabled`, exactly like the CLI. For a whole-book / CI run, use
`inkhaven docs verify`.

---

## Configuration

The `docs:` block. Verification is the only part with runtime behaviour; the rest of
the block carries sibling documentation features (`docs.html` HTML export,
`docs.index` back-of-book index) and the single-sourcing variables below.

```hjson
docs: {
  verify: {
    enabled: false            # master switch — nothing runs unless true
    timeout_seconds: 30       # per-block wall-clock cap
    runners: {                # language → shell command, run via `sh -c`
      rust:   "rustc --edition 2021 --crate-type lib {file} -o {dir}/out"
      python: "python3 {file}"
    }
    # extensions: seeded (rust→rs, python→py, sh→sh, go→go, …); unknown → .txt
  }
  variables: {                # TDOC-3 single-sourcing
    version: "1.6.13"
  }
}
```

- **`runners`** maps a fence language to a command. `{file}` is replaced by the temp
  file holding the block's code, `{dir}` by its parent directory. A language with no
  runner is *skipped*, never failed.
- **`extensions`** maps a language to the temp-file suffix (seeded with the common
  languages; overridable; unknown languages fall back to `.txt`).
- **`variables`** (TDOC-3) — `{{key}}` anywhere in a paragraph body is replaced by its
  value at export assembly, across every export format. This is single-sourcing (put
  the current version number in one place), applied at build time — not a `docs`
  subcommand.

---

## What it catches

| Problem | Which check |
| ------- | ----------- |
| A code example that no longer compiles / runs against the current toolchain | `docs verify` (fail) |
| An internal cross-reference to a renamed or deleted paragraph | `docs links` (always) |
| An external URL gone `404`/`410` — link-rot | `docs links --external` |
| A section still below release readiness | `docs review --floor …` |
| A section that changed since the last tag and wants a re-read | `docs review --since <ref>` |

---

## What it is not

- Not AI — the core is deterministic; the only network touch is the opt-in external
  link sweep, and no model is ever consulted.
- Not an autopilot — `docs verify` runs nothing until `docs.verify.enabled` is `true`
  *and* you pass `--yes`; it only ever runs commands you configured yourself.
- Not a rewriter — every TDOC check reports; none edit prose.
- Not a linter for arbitrary code — it verifies only blocks you marked `verify`, in
  languages you gave a runner.
