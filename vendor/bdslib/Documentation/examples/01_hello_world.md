# 01_hello_world.bund

**File:** `examples/01_hello_world.bund`

Introduction to the BUND VM: pushing values onto the stack and printing them.

## What it demonstrates

- Pushing string literals onto the stack
- Pushing integer and boolean literals
- Using `println` to print and pop the top of stack
- The fundamental push-then-operate execution model

## Key words used

| Word | Effect |
|---|---|
| `"string"` | Push a string literal onto the stack |
| `42` | Push an integer literal |
| `true` / `false` | Push boolean literals |
| `println` | Pop and print the top of stack, followed by a newline |

## Concepts

Every BUND program operates on an implicit data stack. Literals are pushed by name; words (operations) consume values from the top of the stack. There are no variable declarations — data flows through the stack.

## Example output

```
Hello, World!
42
true
```
