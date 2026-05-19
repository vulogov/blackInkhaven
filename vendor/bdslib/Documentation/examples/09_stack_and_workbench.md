# 09_stack_and_workbench.bund

**File:** `examples/09_stack_and_workbench.bund`

Advanced stack manipulation: the workbench, named stacks, and function pointers.

## What it demonstrates

- The workbench (`.`): a secondary holding area separate from the main stack
- `+.` / `print.`: push to and print from the workbench
- `dup` / `swap` / `drop`: fundamental stack reordering
- `@name`: named stacks as a lightweight variable system
- `execute`: invoke a function reference at runtime
- Backtick (`` ` ``): push a function pointer without calling it

## Key words used

| Word | Effect |
|---|---|
| `.` | Move the top of stack to the workbench |
| `+.` | Move the top of the workbench back to the stack |
| `print.` | Print and discard the workbench contents |
| `dup` | Duplicate the top of stack |
| `swap` | Swap the top two stack items |
| `drop` | Discard the top of stack |
| `@name push` | Push a value onto the named stack `name` |
| `@name pop` | Pop a value from named stack `name` onto the main stack |
| `` `word `` | Push a reference to `word` without calling it |
| `execute` | Pop a function reference and call it |

## Concepts

The workbench acts as a one-deep scratchpad for temporarily holding a value while performing operations on others. Named stacks (`@name`) provide a persistent, per-name LIFO store — effectively lightweight variables that work with stack semantics.

Backtick + `execute` enables higher-order patterns: you can pass functions as values, store them in lists, and call them later.

## Example

```
42 .            # stash 42 in the workbench
100 200 +       # compute 300 on main stack
+.              # retrieve 42
println         # prints 42
```
