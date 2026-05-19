# 06_lists.bund

**File:** `examples/06_lists.bund`

Building and processing lists: head/tail decomposition, indexing, and higher-order operations.

## What it demonstrates

- Constructing lists with `[ ... ]`
- `car` / `cdr`: head and tail of a list (Lisp-style)
- `head` / `tail`: first / all-but-first
- `at`: index into a list
- `len`: length of a list
- `push`: append to a list
- `map`: transform a list
- Recursive list processing (sum using `car`/`cdr`)

## Key words used

| Word | Effect |
|---|---|
| `[ a b c ]` | Push a list literal containing a, b, c |
| `car` | Pop list; push its first element |
| `cdr` | Pop list; push the tail (all but first) |
| `head` | Synonym for `car` |
| `tail` | Synonym for `cdr` |
| `at N` | Pop list; push element at index N (0-based) |
| `len` | Pop list; push its length |
| `push` | Pop value and list; push list with value appended |
| `map { block }` | Apply block to each element; push new list |

## Concepts

BUND lists are immutable values — `push` produces a new list rather than mutating. `car`/`cdr` are standard functional programming idioms for recursive list traversal. Combined with named functions and conditionals, they enable recursive algorithms like sum, filter, and reduce.

## Example: recursive sum

```
:sum-list {
    dup len 0 == if { drop 0 return }
    dup car . cdr sum-list +
} register

[ 1 2 3 4 5 ] sum-list println   => 15
```
