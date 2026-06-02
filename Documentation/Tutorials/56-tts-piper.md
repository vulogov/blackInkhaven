# Tutorial 56 — TTS Piper (cross-platform text-to-speech)

*Inkhaven 1.2.17+*

1.2.9 shipped a macOS-only read-aloud feature wired to
`/usr/bin/say`.  1.2.17 lifts that into a backend-agnostic
engine that prefers [Piper](https://github.com/rhasspy/piper)
(neural TTS, cross-platform, voices stored per project)
and falls back to the 1.2.9 System backend when Piper
isn't available.

## Quick start

```hjson
{
  editor: {
    tts: {
      enabled: true
      engine: auto                  // piper if resolvable, else system
      voice: en_US-lessac-medium    // Piper voice key OR macOS say voice
      speed: 1.0
    }
  }
}
```

Then:

* `Ctrl+B S` — read the open paragraph aloud.
* `Ctrl+B Shift+R` — save the paragraph as an audio file.
* `Ctrl+B Shift+V` — open the voice picker (1.2.17+).
* `ink.tts.speak` — Bund word, same engine.

The chord names + behaviour are unchanged from 1.2.9; only
the backend selection changed.

## Engine resolution

`tts.engine` accepts three values:

| Value      | Behaviour |
|------------|-----------|
| `"auto"` (default) | Try to resolve Piper first; fall back to System if Piper's binary isn't on `PATH` or in the user cache.  The 1.2.9 macOS `say` path stays intact. |
| `"piper"`  | Force Piper.  Errors at startup if the binary can't be resolved — no fall-through. |
| `"system"` | Force the 1.2.9 backend.  Errors on non-macOS hosts. |

The status-bar chip (and `inkhaven tts engine` from the
CLI) tells you which backend is currently active:

```
$ inkhaven tts engine
inkhaven TTS engine — v1.2.17
project:       /home/me/Books/my-novel
master switch: enabled
requested:     tts.engine = "auto"
voice:         en_US-lessac-medium
speed:         1
platform:      linux-x86_64
piper cache:   /home/me/.cache/inkhaven
piper binary:  /home/me/.cache/inkhaven/piper-linux-x86_64/piper

→ effective backend: Piper (auto)
```

## Bringing Piper online (first run)

On a fresh machine `engine: "auto"` will fall through to
System because no Piper binary is installed yet.  Two
ways to bring it online:

### From the TUI — the voice picker

`Ctrl+B Shift+V` opens the **voice picker** modal:

```text
 ┌ Piper voices ──────────────────────────────────────────────┐
 │ catalog: fresh · 124 voice(s)                              │
 │   filter: /en (37 match)                                   │
 │                                                            │
 │  › ✓ en_US-lessac-medium       English (en_US)  medium    60 MB │
 │      ⬇ en_US-ryan-high          English (en_US)  high     104 MB │
 │      ⬇ en_GB-alba-medium        English (en_GB)  medium    60 MB │
 │      ⬇ ru_RU-irina-medium       Russian (ru_RU)  medium    60 MB │
 │      ⬇ fr_FR-tom-medium         French  (fr_FR)  medium    60 MB │
 │      ...                                                   │
 │                                                            │
 │  ↑↓ select · / filter · Enter download/use · d remove · Esc │
 └────────────────────────────────────────────────────────────┘
```

* Type characters to filter (matches voice key, language
  code, or English language name; case-insensitive).
* `↑↓` / `PgUp` / `PgDn` / `Home` / `End` to navigate.
* `Enter` on a `⬇ available` row — downloads the voice
  (blocking, ~5–30 s on a fast connection) + sets the
  runtime voice + closes.
* `Enter` on a `✓ downloaded` row — sets the runtime
  voice + closes.
* `d` on a downloaded row (filter must be empty) —
  removes the voice from disk + drops it from the LRU.
* `Esc` closes.

The voice picker doesn't yet rewrite `inkhaven.hjson` —
`tts.voice` changes are session-local.  Use
`inkhaven config` to make it persistent (1.2.10+ HJSON
editor).

### From the CLI

```bash
$ inkhaven tts binary download
inkhaven Piper binary download — v1.2.17
platform:   linux-x86_64
cache root: /home/me/.cache/inkhaven
fetching from GitHub Releases ... OK
installed:  /home/me/.cache/inkhaven/piper-linux-x86_64/piper (... bytes)

$ inkhaven tts voice download en_US-lessac-medium
inkhaven voice download — en_US-lessac-medium
voices_dir: /home/me/Books/my-novel/.inkhaven/voices
downloading ... OK
onnx:       .../en_US-lessac-medium.onnx
onnx.json:  .../en_US-lessac-medium.onnx.json

$ inkhaven tts test "Hello world"
inkhaven TTS test (engine-routed) — v1.2.17
project:  /home/me/Books/my-novel
phrase:   "Hello world"
voice:    en_US-lessac-medium
engine:   auto
resolved: piper
synthesising + playing ... OK
```

## On-disk layout

```text
~/.cache/inkhaven/                       # user-scoped binary cache
  piper-linux-x86_64/
    piper                                # the executable
    espeak-ng-data/                      # 400 phoneme tables
    piper_phonemize                      # helper binary
    libtashkeel_model.ort                # Arabic diacritic model
    ...

<project>/.inkhaven/voices/              # per-project voice cache
  voices.json                            # catalog (24h TTL)
  .lru                                   # access-time index
  en_US-lessac-medium.onnx               # ~63 MB
  en_US-lessac-medium.onnx.json          # ~5 KB
  ru_RU-irina-medium.onnx
  ru_RU-irina-medium.onnx.json
```

Two scopes on purpose:

* The **Piper binary** is identical across every project
  on the same machine, so it lives in the user cache.
* **Voices** are project-specific — a French-novel
  project wants `fr_FR-*`; a Russian short-story project
  wants `ru_RU-*`.  Storing them per project keeps the
  cache focused.

On first voice download inkhaven appends
`.inkhaven/voices/` to the project's `.gitignore`
(idempotent; creates the file if absent).  Disable via
`tts.auto_gitignore: false` if you manage `.gitignore`
strictly by hand.

## Voice naming

Piper voice keys follow the shape `<lang>-<name>-<quality>`:

| Component | Meaning | Example |
|-----------|---------|---------|
| `lang`    | BCP-47-ish locale (underscores, not dashes) | `en_US`, `ru_RU`, `fr_FR` |
| `name`    | Speaker name | `lessac`, `irina`, `ryan` |
| `quality` | Tier        | `x_low`, `low`, `medium`, `high` |

Higher quality → larger model + slower synthesis.  The
voice picker sorts by language then quality-descending, so
the best voice for each language lands first.

## CLI surface

```bash
$ inkhaven tts engine                            # backend status

$ inkhaven tts binary status                     # piper binary info
$ inkhaven tts binary download                   # explicit binary fetch

$ inkhaven tts voice list                        # catalog + downloaded
$ inkhaven tts voice list --filter ru            # filter by language / name
$ inkhaven tts voice list --downloaded           # show only what's on disk
$ inkhaven tts voice download <name>             # explicit voice fetch
$ inkhaven tts voice remove <name>               # delete + update LRU

$ inkhaven tts catalog refresh                   # bypass 24h TTL

$ inkhaven tts test "<phrase>"                   # synth + play
$ inkhaven tts test "<phrase>" --voice <name>    # voice override
$ inkhaven tts test "<phrase>" --output out.wav  # synth without playing
```

All output is line-oriented + grep-friendly.  The CLI
runs synchronously (`tts test` waits for playback to
finish before exiting) so scripts can chain operations.

## HJSON reference

The full `tts.*` block (defaults shown):

```hjson
{
  editor: {
    tts: {
      // 1.2.9+
      enabled: false               // master switch
      voice: "Milena"              // voice needle (engine-specific)
      speed: 1.0                   // 1.0 = normal, 0.8 = 80% etc
      greeting: ""                 // spoken at TUI startup
      goodbye: ""                  // spoken at TUI shutdown (blocking, 5s cap)

      // 1.2.17+
      engine: "auto"               // "auto" | "piper" | "system"
      voices_dir: ".inkhaven/voices"  // relative to project root
      auto_download: true          // fetch missing voices on first use
      catalog_url: "https://huggingface.co/rhasspy/piper-voices/raw/main/voices.json"
      catalog_ttl_hours: 24
      binary_path: null            // null = autoresolve via PATH + cache
      auto_download_binary: true   // (for the CLI; startup never auto-downloads)
      cache_max_voices: 5          // LRU eviction past this count
      play_command: null           // {path} placeholder; null = platform default
      sample_rate_hz: 22050        // Piper native rate
      auto_gitignore: true         // append .inkhaven/voices/ on first download
    }
  }
}
```

See `Documentation/CONFIGURATION.md` for the per-field
reference.

## Playback dispatch

| Platform | Default `tts.play_command` |
|----------|----------------------------|
| macOS    | `afplay {path}`            |
| Linux    | `paplay {path}` → falls back to `aplay {path}` if PulseAudio isn't installed |
| Windows  | `powershell -NoProfile -Command "(New-Object Media.SoundPlayer '{path}').PlaySync()"` |

Override with `tts.play_command: "mpv --no-video {path}"`
(or `ffplay -nodisp -autoexit {path}`, or `sox {path} -d`,
etc.).  The `{path}` placeholder is substituted at spawn
time; no shell intermediary, so quoting + escaping work
the same way `Command::new` always does.

## Known limitation — Apple Silicon Piper

Piper's official release pipeline ships an
`piper_macos_aarch64.tar.gz` asset that **actually
contains x86_64 code**, not aarch64.  This is a known
upstream packaging bug (verify with `file` on the
extracted binary).  On Apple Silicon Macs the binary
loads but can't link against Homebrew's arm64
`libespeak-ng.1.dylib`, so synthesis fails.

Three workarounds:

* `tts.engine: "system"` — falls back to macOS `say`,
  which works perfectly + ships dozens of high-quality
  voices.  Inkhaven 1.2.9 was built around this backend
  and the integration is mature.
* **Build Piper from source** for arm64 then set
  `tts.binary_path: "/usr/local/bin/piper"` (or
  wherever your build lands).
* **Run x86_64 Piper under Rosetta** with x86_64
  espeak-ng installed at `/usr/local/lib/`.  Requires
  setting up Intel Homebrew alongside Apple Silicon
  Homebrew.

Linux + Windows have no such issue — `engine: "auto"`
resolves cleanly to Piper.

## Status-bar chip

When `tts.enabled = true`, the status bar shows a small
chip indicating the active engine (`tts: piper` /
`tts: system` / `tts: disabled`).  Useful for confirming
the resolver picked what you expected without leaving the
TUI.

## Troubleshooting

### "TTS unavailable" modal on `Ctrl+B S`

Run `inkhaven tts engine` from the CLI.  Common causes:

* `master switch: disabled` — set `tts.enabled = true`
  in `inkhaven.hjson`.
* `piper binary: not found on PATH or in cache` — run
  `inkhaven tts binary download`.
* `tts.engine = "system"` on a non-macOS host — switch
  to `"auto"` or `"piper"` + download a voice.

### Voice downloaded but synthesis still fails

Verify the voice files landed at the expected path:

```bash
$ ls <project>/.inkhaven/voices/
en_US-lessac-medium.onnx
en_US-lessac-medium.onnx.json
voices.json
.lru
```

Both `.onnx` and `.onnx.json` must be non-empty (a
zero-byte file from an interrupted download is treated
as "not present" — re-run `tts voice download <name>`).

### Voice cache growing too large

Voice models are 25–100 MB each; `tts.cache_max_voices`
caps the project's voices directory.  When the count
exceeds the cap, the least-recently-used voice is
evicted (`.onnx` + `.onnx.json` removed; the LRU index
updated).  Default cap is 5 voices.

Lower the cap to reclaim disk:

```hjson
{ editor: { tts: { cache_max_voices: 2 } } }
```

Or remove individual voices:

```bash
$ inkhaven tts voice remove en_US-ryan-high
removed 2 file(s) for voice `en_US-ryan-high`
```

### Catalog refresh fails offline

If `inkhaven tts catalog refresh` errors (network down),
the existing cache stays in place; the CLI exits non-
zero.  Synthesis with already-downloaded voices keeps
working — only catalog browsing + new voice downloads
need the network.

The TUI picker (`Ctrl+B Shift+V`) handles this
gracefully: when the catalog can't be fetched + a stale
cache exists, the picker opens with a `catalog: stale`
header.  When neither catalog nor cache is available,
the picker falls back to listing voices already on disk
with a `catalog: offline` header.

## See also

* `Ctrl+B S` — read aloud (1.2.9+).
* `Ctrl+B Shift+R` — save as audio (1.2.9+).
* `Ctrl+B Shift+V` — voice picker (1.2.17+).
* `ink.tts.speak` — Bund word, same engine.
* `Documentation/PROPOSALS/1.2.17_PLAN.md` — design
  document + phase plan.
* `Documentation/RELEASE_NOTES/1.2.17.md` —
  implementation log.
