# 02_arithmetic.bund

**File:** `examples/02_arithmetic.bund`

Stack-based arithmetic: integers, floats, and bulk summation.

## What it demonstrates

- Basic arithmetic words (`+`, `-`, `*`, `/`, `%`)
- Float operations: `float.sqrt`, `float.Pi`
- The `*+` word for summing all values on the stack at once
- Integer vs. float type promotion

## Key words used

| Word | Effect |
|---|---|
| `+` `-` `*` `/` `%` | Standard arithmetic on the top two stack items |
| `float.sqrt` | Replace top of stack with its square root |
| `float.Pi` | Push π (3.14159…) |
| `*+` | Sum all items currently on the stack into a single value |

## Concepts

BUND arithmetic is postfix (Reverse Polish Notation). To compute `3 + 4`, push `3`, push `4`, then call `+`. The two operands are consumed and the result is pushed.

`*+` is a fold operation — it collapses the entire stack into a single sum, useful when building totals incrementally.

## Example output

```
7          (3 + 4)
3.1415...  (float.Pi)
1.414...   (sqrt 2)
15         (*+ of 1 2 3 4 5)
```
