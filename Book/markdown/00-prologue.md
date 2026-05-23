# Prologue — Why this book exists

Many writers have a process that survives them — a way of moving through a manuscript that the next book inherits without anyone teaching it. Inkhaven exists for that process: a quiet, keyboard-driven editor that holds the manuscript as a structured tree, talks to large language models without leaking your private prose, renders to Typst PDFs, and stays out of your way while you write.

This book is a guided tour through every feature, in the order a working novelist would actually meet them. It begins with installing the binary and writing a single paragraph; it ends with scripting the project from a custom Bund hook that checks your timeline for consistency on every save.

## How to read it

Read in order if you are new. The early chapters are deliberately bare — open a project, type a sentence, save — because everything later assumes you've felt those motions in your fingers. By the time you reach Part IV on world building you should be able to navigate without thinking about the chord chart.

Skip around if you've been using inkhaven for a while. Each chapter is self-contained: it tells you what's new in its corner of the system, lists the relevant chords, shows a figure of the screen the chord produces, and ends with a recap you can scan in a minute. Cross-references point you sideways when something interlocks.

## Conventions

Keystrokes are written `Ctrl+S`, `F1`, `Ctrl+B` then `P` (the meta prefix needs two keypresses). Code blocks contain prose you type into a file or commands you run at a shell — the surrounding text says which.

Every figure is a placeholder: a numbered slot the book fills with a real terminal screenshot. The catalog in `Book/SCREENSHOTS.md` lists every figure, the chord state it captures, and the terminal size + theme it expects.

## What this book is not

**Not a Typst reference.** The Typst language has its own excellent documentation at typst.app/docs; we'll show you what the typesetter does when inkhaven sends prose to it, not how the typesetter itself works.

**Not a tutorial on writing.** We assume you bring the prose; inkhaven is the room you write it in.

**Not exhaustive about Bund.** Chapter 29 introduces the language and shows the inkhaven-specific stdlib; the canonical reference at `Documentation/Bund/BUND_TUTORIAL.md` goes deeper.

## A note on AI

Inkhaven is built to use large language models as writing partners — for grammar review, critique, RAG search, and timeline auditing. The book describes those features honestly: when they help, when they get in the way, and how to turn them off when the prose needs your full attention.

Every AI surface is opt-in or scope-limited. Inkhaven never sends a paragraph to a remote provider without you pressing a chord. The full-screen AI mode (`Ctrl+B K`) and the per-paragraph memory (`ai.per_paragraph_memory`) are also opt-in.

But — and this is important — inkhaven does *not* provide inherent privacy when you use one of the cloud LLM providers (Gemini, Claude, OpenAI, DeepSeek, Grok). Every chord that talks to those providers sends prose to their servers under their terms of service. They may log it, train on it, or otherwise retain it. Chapter 18 covers this in detail.

For privacy, install a local Ollama instance and set `llm.default_provider: "ollama"`. Every non-LLM AI surface in inkhaven is already on-device; Ollama closes the last loop. If you write entirely without AI, the configured provider is irrelevant — none of inkhaven's non-AI features need an LLM.

*Now — let's begin.*
