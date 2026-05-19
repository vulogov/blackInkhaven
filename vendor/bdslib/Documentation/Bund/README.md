# BUND Language Documentation

BUND is a stack-based, concatenative programming language with a circular multistack virtual machine and dynamic typing. This directory contains the complete language reference.

---

## Documents

### Language Reference

| Document | Format | Contents |
|---|---|---|
| [SYNTAX_AND_VM.md](SYNTAX_AND_VM.md) | Markdown | Full language reference: syntax, VM architecture, all data types, all built-in operations |
| [SYNTAX_AND_VM.typ](SYNTAX_AND_VM.typ) | Typst | Typeset version of the above — render with `typst compile SYNTAX_AND_VM.typ` |

### Standard Library Reference

| Document | Format | Contents |
|---|---|---|
| [BASIC_LIBRARY.md](BASIC_LIBRARY.md) | Markdown | Complete word reference with stack-effect notation for every built-in command |
| [BASIC_LIBRARY.typ](BASIC_LIBRARY.typ) | Typst | Typeset version of the above — render with `typst compile BASIC_LIBRARY.typ` |

### Extension Libraries

| Document | Format | Contents |
|---|---|---|
| [BDS.md](BDS.md) | Markdown | All `db.*` and `doc.*` words — shard DB ingest/search and document store operations |

---

## Examples

The [`../../examples/`](../../examples/) directory contains 10 annotated BUND programs:

| File | Topic |
|---|---|
| `01_hello_world.bund` | Printing, basic string output |
| `02_arithmetic.bund` | Numbers, arithmetic, workbench |
| `03_control_flow.bund` | `if`, `ifthenelse`, `while`, `for` |
| `04_functions.bund` | `register`, recursion, aliases |
| `05_lists.bund` | List construction, `car`/`cdr`, `map`, `loop` |
| `06_dictionaries.bund` | Map creation, `set`/`get`/`has_key` |
| `07_strings.bund` | Case conversion, pattern matching, regex |
| `08_maps_and_types.bund` | Type inspection, `type`, `convert.*` |
| `09_stack_and_workbench.bund` | Named stacks, workbench patterns, `dup`/`swap` |
| `10_full_program.bund` | Statistics tool combining all features |

---

## Quick Start

```bund
// Hello, World
"Hello, BUND!" println

// Arithmetic (left-to-right, no precedence)
3 4 + 2 *  println      // 14

// Named function
:square { dup * } register
5 square println         // 25

// List and map
[ 1 2 3 4 ] { 2 * } map println  // [ 2 4 6 8 ]
```

---

## Key Concepts

**Stack model** — Values are pushed left to right; commands consume from the top and push results. There are no variables; the stack *is* the program state.

**Workbench** — A scratchpad register separate from all stacks. The `.` command moves the top of the stack to the workbench; commands with a `.` suffix read their argument from the workbench instead.

**Named stacks** — `@name` switches the active stack. Multiple independent stacks coexist in a ring; any stack can be addressed by name at any time.

**Lambdas** — `{ … }` creates an inert code block pushed as a value. Lambdas are executed by control-flow words (`if`, `while`, `map`, `do`, …) or by naming them with `register`.

**Dynamic types** — Every value carries its type at runtime. The 33 built-in types range from `INTEGER` and `FLOAT` through `LIST`, `MAP`, `MATRIX`, `JSON`, and `EMBEDDING` to meta-types like `LAMBDA`, `CLASS`, and `OBJECT`.

---

## Rendering the Typst Documents

Install [Typst](https://typst.app/) and run:

```sh
typst compile Documentation/Bund/SYNTAX_AND_VM.typ
typst compile Documentation/Bund/BASIC_LIBRARY.typ
```

This produces `SYNTAX_AND_VM.pdf` and `BASIC_LIBRARY.pdf` in the same directory.
