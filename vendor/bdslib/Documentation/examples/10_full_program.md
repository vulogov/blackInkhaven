# 10_full_program.bund

**File:** `examples/10_full_program.bund`

A complete statistics program demonstrating all major BUND features together.

## What it demonstrates

- Defining multiple cooperating named functions
- Computing sum, max, min over a list using recursion
- Classifying a value relative to a threshold
- Formatted output using the workbench and string operations
- Real-world structure: functions defined first, program logic at the bottom

## Functions defined

| Function | Description |
|---|---|
| `sum_list` | Recursively sum all elements in a list |
| `max_list` | Find the maximum element in a list |
| `min_list` | Find the minimum element in a list |
| `classify` | Given a value and threshold, push "above" or "below" |
| `print_stat` | Print label, value, and classification in one line |

## Program flow

1. Define helper functions
2. Push a sample dataset as a list literal
3. Compute sum, mean (sum / len), max, and min
4. Classify each metric against a fixed threshold
5. Print formatted results using `print_stat`

## Concepts

This example is the capstone of the BUND tutorial series. It shows that real programs are built by composing small stack-friendly functions. The key discipline is thinking in terms of what each word leaves on the stack — every function is documented by its stack effect.

## Example output

```
sum:  385
mean: 38.5 above
max:  99   above
min:  1    below
```
