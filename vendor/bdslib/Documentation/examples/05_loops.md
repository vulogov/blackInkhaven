# 05_loops.bund

**File:** `examples/05_loops.bund`

Iteration with `times`, `do`, `map`, `while`, and `for`.

## What it demonstrates

- `times`: repeat a block N times
- `do`: iterate over a list, executing a block for each element
- `map`: transform a list by applying a block to each element
- `while`: loop as long as a condition is true
- `for`: index-based iteration
- Fibonacci sequence as a real loop example

## Key words used

| Word | Effect |
|---|---|
| `N times { block }` | Execute block N times |
| `list do { block }` | Iterate over list; block receives each element |
| `list map { block }` | Build a new list by applying block to each element |
| `while { cond } { body }` | Loop while condition block leaves `true` on stack |
| `for start end { block }` | Iterate from start to end (exclusive), pushing index |

## Concepts

`times` is a counted loop with no index. `do` is a for-each over a list. `map` is a higher-order transform that produces a new list — the block must leave exactly one value on the stack per call. `while` separates the condition and body into two blocks for clarity.

## Example: fibonacci

```
:fib {
    1 1
    10 times { dup . + }
} register

fib       => stack has 10 fibonacci numbers
```

## Example output

```
looping 3 times
1 2 3 4 5   (for loop 1..5)
```
