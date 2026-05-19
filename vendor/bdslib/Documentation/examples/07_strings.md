# 07_strings.bund

**File:** `examples/07_strings.bund`

String manipulation: case conversion, pattern matching, regex, and tokenization.

## What it demonstrates

- Case conversion: `string.upper`, `string.lower`
- Wildcard matching: `string.wildmatch`
- Regular expression matching: `string.regex`
- Tokenization: `string.tokenize`
- String literals with special characters

## Key words used

| Word | Effect |
|---|---|
| `string.upper` | Pop string; push uppercased version |
| `string.lower` | Pop string; push lowercased version |
| `string.wildmatch pattern` | Pop string; push true if it matches the glob pattern |
| `string.regex pattern` | Pop string; push true if it matches the regex |
| `string.tokenize` | Pop string; push a list of whitespace-delimited tokens |

## Concepts

`string.wildmatch` uses shell-style glob patterns (`*`, `?`, `[abc]`). `string.regex` uses full regular expressions. Both return a boolean on the stack, which can be consumed by `if` / `ifthenelse`.

`string.tokenize` is useful for splitting log messages or input lines into fields.

## Example

```
"Hello World" string.upper println        => HELLO WORLD
"file.log" string.wildmatch "*.log" if { "is a log" println }
"error in module" string.tokenize len println  => 3
```
