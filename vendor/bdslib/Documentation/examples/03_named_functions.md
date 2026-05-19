# 03_named_functions.bund

**File:** `examples/03_named_functions.bund`

Defining, aliasing, and calling named functions including recursive ones.

## What it demonstrates

- Defining a function with `:name { body } register`
- Creating an alias with `alias`
- Recursive functions (factorial)
- Functions as first-class values

## Key words used

| Word | Effect |
|---|---|
| `:name { body } register` | Define and register a named function |
| `alias` | Create a second name for an existing function |
| `call` | Invoke a function by name from the stack |

## Concepts

Named functions are defined by pushing a function literal (`:name { ... }`) and then calling `register`, which installs the function into the VM's word dictionary. After registration, the function can be called by name like any built-in word.

`alias` lets you bind an existing word to a new name without redefining the body. Recursion works naturally: a function can call itself by name since the dictionary is resolved at call time.

## Example: factorial

```
:factorial {
  dup 1 <= if.false { dup 1 - factorial * }
} register

5 factorial println    => 120
```

## Example output

```
hello from named function
120   (5 factorial)
```
