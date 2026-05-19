# 04_conditionals.bund

**File:** `examples/04_conditionals.bund`

Branching with `if`, `if.false`, `ifthenelse`, and boolean combinators.

## What it demonstrates

- `if`: execute a block only when the top of stack is `true`
- `if.false`: execute a block only when the top of stack is `false`
- `ifthenelse`: two-branch conditional (pop condition, then-block, else-block)
- Comparison words: `<`, `>`, `<=`, `>=`, `==`, `!=`
- Boolean combinators: `and`, `or`, `not`
- Type predicate: `?type`

## Key words used

| Word | Effect |
|---|---|
| `if` | Pop condition; if true, execute the next block |
| `if.false` | Pop condition; if false, execute the next block |
| `ifthenelse` | Pop condition; execute then-block if true, else-block if false |
| `<` `>` `<=` `>=` `==` `!=` | Compare top two stack items; push boolean |
| `and` `or` `not` | Boolean logic on top of stack |
| `?type` | Push the type name of the top-of-stack value as a string |

## Concepts

BUND conditionals consume the boolean from the top of the stack, so comparisons must be placed before the conditional word. `ifthenelse` takes three values: the condition (bottom), the then-block, and the else-block (top).

`?type` is useful for runtime type dispatch — it pushes a string like `"Int"`, `"Text"`, `"Bool"`.

## Example

```
10 5 > if { "ten is greater" println }   => ten is greater
3 7 == if.false { "not equal" println }  => not equal
```
