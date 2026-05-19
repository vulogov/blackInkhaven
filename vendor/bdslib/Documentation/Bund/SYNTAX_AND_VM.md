# BUND Language — Syntax, Virtual Machine, and Data Types

## Table of Contents

1. [Language Overview](#1-language-overview)
2. [Execution Model — The Circular MultiStack VM](#2-execution-model)
3. [Syntax Reference](#3-syntax-reference)
4. [Data Types](#4-data-types)
5. [Stack Operations](#5-stack-operations)
6. [Arithmetic](#6-arithmetic)
7. [Logic and Comparison](#7-logic-and-comparison)
8. [Control Flow](#8-control-flow)
9. [Lambda Functions](#9-lambda-functions)
10. [Collections — Lists, Maps, Matrices](#10-collections)
11. [String Operations](#11-string-operations)
12. [Type Conversion](#12-type-conversion)
13. [Time and Timestamps](#13-time-and-timestamps)
14. [JSON Integration](#14-json-integration)
15. [Output and I/O](#15-output-and-io)
16. [Standard Library Quick Reference](#16-standard-library-quick-reference)

---

## 1. Language Overview

BUND is a **stack-based, concatenative programming language**. Programs are sequences of values and operations written left to right. When the interpreter encounters a value (a number, string, list, etc.) it is pushed onto the current stack. When it encounters a command name, that command is executed immediately, typically consuming values from the top of the stack and pushing results.

There is no assignment syntax, no variables in the conventional sense, and no operator precedence — all execution follows strict left-to-right order determined by the order of tokens in source code.

### Core design principles

- **Everything is a value.** Numbers, strings, lambdas, commands, and even type descriptors are all first-class values that can be pushed, popped, and passed around.
- **Code is data.** A lambda `{ ... }` is just a value of type `LAMBDA` sitting on the stack until something executes it.
- **Multiple independent stacks.** The VM maintains several named stacks that can be addressed by name; execution can switch between them.
- **Workbench registers.** A special "workbench" area provides fast temporary storage separate from the main stack.
- **Dynamic typing.** Values carry their type with them at runtime; no type declarations are needed.

---

## 2. Execution Model

### 2.1 The Circular MultiStack VM

The BUND Virtual Machine is a *circular multistack* engine. Rather than a single stack shared by the entire program, the VM maintains a collection of named stacks arranged logically in a ring. At any moment one stack is *current*; execution pushes and pops values on that stack. Switching to a different stack is an explicit operation, and the switch is always available by name.

```
          ┌──────────────┐
          │   "main"     │  ← default stack at start
          │  stack       │
          └──────┬───────┘
                 │  switch via context
     ┌───────────┴────────────┐
     ▼                        ▼
┌──────────┐            ┌──────────┐
│ "worker" │            │ "result" │
│  stack   │            │  stack   │
└──────────┘            └──────────┘
```

All stacks are last-in-first-out (LIFO). A named stack is created on first reference if `autoadd` mode is enabled in the VM.

### 2.2 The Workbench

In addition to the stacks, the VM provides a **workbench** — a set of named registers that hold values outside of any stack. Many standard operations have a `.`-suffixed variant (for example `println.`) that reads its input from the workbench instead of the current stack, and the `.` command itself moves the top of the current stack into the workbench.

```
Current stack:     top → [ 42 | "hello" | 3.14 | ... ]
                             ↓  (. command)
Workbench:                  42
```

### 2.3 Applying a Token

The central operation of the VM is `apply(value)`. Its behaviour depends on the type of the value being applied:

| Value type | Behaviour |
|---|---|
| Integer, Float, String, Literal, Binary, List, Map, Matrix | Push directly onto the current stack (or workbench if in autoadd mode) |
| `CALL` (command name) | Look up the name: execute built-in, registered lambda, or inline function |
| `PTR` (backtick reference) | Look up and push the referenced value without executing it |
| `LAMBDA` | Push the code block onto the stack — it is *not* auto-executed |
| `CONTEXT` | Switch the current stack to the named context |
| `EXIT` | Halt the current execution sequence |
| `ERROR` | Raise an error, halting execution |

### 2.4 Autoadd Mode and the `:` / `;` Delimiters

The `:` token enables **autoadd mode**. While active, values and commands encountered in the source are collected into a working structure rather than being executed immediately. The `;` token closes autoadd mode and finalises the structure.

This pair is primarily used internally for lambda registration patterns. When you write:

```bund
:FourtyTwo { 42 } register
```

The atom `:FourtyTwo` is the name that will be bound to the lambda `{ 42 }`, and `register` stores it. The `:` here is the atom prefix, not the autoadd delimiter — atoms begin with `:` and end with whitespace and appear as a named label/identifier.

### 2.5 Evaluation Order

BUND code is evaluated strictly left to right. There is no look-ahead, no operator precedence, and no implicit re-ordering. The statement:

```bund
3 4 + 2 *
```

executes as:
1. Push `3`
2. Push `4`
3. Execute `+` → pops `4` and `3`, pushes `7`
4. Push `2`
5. Execute `*` → pops `2` and `7`, pushes `14`

Result on stack: `14`

---

## 3. Syntax Reference

### 3.1 Comments

Single-line comments begin with `//` and continue to the end of the line.

```bund
// This is a comment
42 println  // inline comment after code
```

### 3.2 Integers

A 64-bit signed integer. Optional leading `+` or `-` sign.

```bund
42
-100
+7
0
```

### 3.3 Floats

A 64-bit floating-point number. Must contain a decimal point; optional sign; optional exponent.

```bund
3.14
-42.5
1.0e-5
2.718281828
```

Special float constants (commands):

```bund
float.Pi        // π ≈ 3.14159…
float.E         // Euler's e ≈ 2.71828…
float.NaN       // Not a Number
float.+Inf      // Positive infinity
float.-Inf      // Negative infinity
```

### 3.4 Strings

Double-quoted UTF-8 text. Standard escape sequences apply.

```bund
"Hello, world!"
"Line one\nLine two"
"A quoted \"word\" inside"
```

### 3.5 Literals

Single-quoted raw text. No escape processing — the content between the quotes is taken verbatim.

```bund
'raw text with no \escapes'
'path/to/something'
```

### 3.6 Atoms

An identifier prefixed with `:` and followed by whitespace. Atoms are used as symbolic names — primarily as labels for lambda registration.

```bund
:my_function
:answer
:HttpHandler
```

The atom token produces a value of type `STRING` whose content is the name after the colon. It is typically consumed by commands like `register` or `alias`.

### 3.7 Commands and Names

Any sequence of non-whitespace, non-special characters that is not a number, atom, pointer, or special form is a **command** or **name**. When the interpreter encounters a command, it is *executed immediately*.

```bund
println          // execute: pop top of stack and print with newline
+                // execute: pop 2 values, push sum
dup              // execute: duplicate top of stack
my_function      // execute: call the registered lambda named "my_function"
```

### 3.8 Pointers (Backtick References)

A name prefixed with a backtick (`` ` ``) pushes a *reference* to the named value onto the stack without executing it. The result is a `PTR`-type value.

```bund
`my_function     // push the lambda value, do NOT call it
`println         // push a reference to the println command
```

Pointers allow lambdas and commands to be stored in data structures or passed as arguments.

### 3.9 Lambdas — `{ … }`

Curly braces delimit a **lambda**: a code block that becomes a `LAMBDA`-type value pushed onto the stack. A lambda is inert until explicitly executed.

```bund
{ 42 println }            // push this code block as a value
{ dup * }                 // a squaring function
{ "hi" println true }     // a lambda that prints and pushes true
```

Lambdas are executed by:
- Calling a registered lambda by name
- Using `do` (executes the lambda on the top of the stack)
- Using `if`, `while`, `for`, `map`, `loop` (control flow operations)

### 3.10 Lists — `[ … ]`

Square brackets delimit a **list**: an ordered, heterogeneous sequence of values. The elements are parsed and stored but not executed.

```bund
[ 1 2 3 ]
[ "alice" "bob" "carol" ]
[ 1 "mixed" 3.14 true ]
[ [ 1 2 ] [ 3 4 ] ]        // nested lists
```

A list is a `LIST`-type value pushed onto the stack.

### 3.11 Contexts — `( … )`

Parentheses create a **context block**. The terms inside are evaluated within a context marker, and when the context closes (`)`) the `endcontext` command is automatically invoked.

```bund
( 41 1 + )   // push context, compute 42, pop context
```

Contexts are used for stack isolation and controlled scope. Code inside `(…)` runs against the current stack; `endcontext` finalises the context boundary.

### 3.12 Named Stacks — `@name`

`@` followed by one or more letters creates a **named stack reference** — a `CONTEXT`-type value whose name is the identifier after `@`. When applied, the VM switches the active stack to that name.

```bund
@worker          // switch current stack to "worker"
@result          // switch current stack to "result"
```

---

## 4. Data Types

Every value in BUND carries a **type discriminant** (a small integer) and **metadata**: a nanoid (`id`), a millisecond timestamp (`stamp`), a quality/confidence metric (`q`, range 0–100+), a position cursor (`curr`), string tags (`tags`), and attribute values (`attr`).

### 4.1 Primitive Types

| Type name | Discriminant | Description |
|---|---|---|
| `NONE` | 0 | Absent/empty. The default zero-value; skipped during execution. |
| `BOOL` | 1 | Boolean: `true` or `false`. |
| `INTEGER` | 2 | Signed 64-bit integer. |
| `FLOAT` | 3 | IEEE 754 64-bit floating-point number. |
| `STRING` | 4 | UTF-8 text string. |
| `LITERAL` | 5 | Raw text literal (single-quoted in source). |

### 4.2 Execution Types

| Type name | Discriminant | Description |
|---|---|---|
| `CALL` | 6 | A command or function name. Causes execution when applied to the VM. |
| `PTR` | 7 | A pointer to a named value. Pushed onto the stack without executing. |
| `LAMBDA` | 17 | A code block `{ … }`. Inert until executed by a control-flow operation. |
| `CONTEXT` | 21 | A named stack reference `@name`. Switches the active stack when applied. |
| `EXIT` | 93 | Signals the interpreter to halt the current execution sequence. |
| `ERROR` | 98 | An error value. Causes the VM to raise an error when applied. |

### 4.3 Collection Types

| Type name | Discriminant | Description |
|---|---|---|
| `LIST` | 9 | An ordered, heterogeneous sequence of values `[ … ]`. |
| `MAP` | 11 | A string-keyed dictionary of values. |
| `VALUEMAP` | 30 | A dictionary whose keys are arbitrary values (not just strings). |
| `QUEUE` | 18 | A FIFO queue. |
| `FIFO` | 19 | Alternative FIFO collection. |
| `MATRIX` | 26 | A 2-dimensional list of values. |

### 4.4 Numeric Extension Types

| Type name | Discriminant | Description |
|---|---|---|
| `CINTEGER` | 14 | Complex number with integer real and imaginary parts. |
| `CFLOAT` | 15 | Complex number with float real and imaginary parts. |
| `LARGE_FLOAT` | 23 | Extended precision floating-point. |

### 4.5 Compound and Structural Types

| Type name | Discriminant | Description |
|---|---|---|
| `PAIR` | 10 | An ordered pair of two values (like a 2-tuple). |
| `ENVELOPE` | 12 | A binary envelope for network/serialisation use. |
| `BIN` | 8 | Raw binary data (byte array). |
| `METRICS` | 16 | A collection of metric measurements. |
| `OPERATOR` | 20 | A wrapped operator value. |
| `TEXTBUFFER` | 22 | A mutable text buffer, built up incrementally. |
| `JSON` | 24 | A native JSON value. |
| `JSON_WRAPPED` | 25 | A BUND value serialised inside a JSON wrapper. |
| `CURRY` | 27 | A partially applied function with captured arguments. |
| `MESSAGE` | 28 | A structured message with `from`, `to`, and `data` fields. |
| `CONDITIONAL` | 29 | A conditional expression value. |
| `CLASS` | 31 | A class definition for object-oriented patterns. |
| `OBJECT` | 32 | An instance of a class. |
| `EMBEDDING` | 33 | A floating-point vector embedding (for ML/similarity work). |

### 4.6 Status and Sentinel Types

| Type name | Discriminant | Description |
|---|---|---|
| `TIME` | 13 | A timestamp value (nanoseconds since epoch). |
| `RESULT` | 92 | A computation result wrapper. |
| `ASSOCIATION` | 94 | An association between two values. |
| `CONFIG` | 95 | A configuration object. |
| `INFO` | 96 | An informational value. |
| `NODATA` | 97 | Explicit "no data available" sentinel. |
| `TOKEN` | 99 | A raw lexer token. |

### 4.7 Value Metadata Fields

Every value, regardless of type, carries the following metadata:

| Field | Type | Description |
|---|---|---|
| `id` | string | Unique nanoid assigned at creation. |
| `stamp` | float | Creation time in milliseconds since epoch. |
| `q` | float | Quality or confidence score (0.0–100.0+). |
| `curr` | integer | Position cursor for iteration. |
| `tags` | map of strings | Arbitrary string-valued tags. |
| `attr` | list of values | Additional attribute values. |

This metadata survives all operations — copying a value preserves its `stamp` and `id`, allowing provenance tracking even after transformation.

---

## 5. Stack Operations

### 5.1 Push and Pop

Values are pushed by simply writing them. Most commands pop their arguments from the stack and push results back.

```bund
42          // stack: [ 42 ]
"hello"     // stack: [ "hello" | 42 ]
+           // ERROR: + expects two numbers — illustrative only
```

### 5.2 Workbench Transfer

The `.` command pops the top of the stack and places it on the workbench. The workbench is independent of all named stacks.

```bund
42 .        // workbench: 42, stack: []
```

Commands that have a `.`-suffix variant read from the workbench instead of the stack:

```bund
42 .        // move 42 to workbench
println.    // print from workbench (prints "42")
```

### 5.3 Duplication

`dup` duplicates the top value on the current stack.

```bund
7 dup       // stack: [ 7 | 7 ]
```

### 5.4 Swap

`swap` exchanges the top two values on the stack.

```bund
1 2 swap    // stack: [ 1 | 2 ] (was [ 2 | 1 ])
```

### 5.5 Length

`len` peeks at the top value (without popping it) and pushes its length as an `INTEGER`. Works on strings, lists, maps, and binary data.

```bund
[ 1 2 3 ] len    // stack: [ 3 | [1 2 3] ]
"hello" len      // stack: [ 5 | "hello" ]
```

### 5.6 List Deconstruction — `car` and `cdr`

`car` pops a list and pushes its first element. `cdr` pops a list and pushes a new list containing all elements except the first.

```bund
[ 10 20 30 ] car    // stack: [ 10 ]
[ 10 20 30 ] cdr    // stack: [ [20 30] ]
```

### 5.7 Element Access — `at`

`at` pops an integer index and a list, and pushes the element at that index (zero-based).

```bund
[ "a" "b" "c" ] 1 at    // stack: [ "b" ]
```

### 5.8 Head and Tail

`head` returns the first N elements; `tail` returns the last N elements.

```bund
[ 1 2 3 4 5 ] 3 head    // stack: [ [1 2 3] ]
[ 1 2 3 4 5 ] 2 tail    // stack: [ [4 5] ]
```

### 5.9 Clearing Stacks

`clear_stacks` removes all values from all stacks.

---

## 6. Arithmetic

All arithmetic operations pop two values from the stack and push one result. They work across compatible numeric types (integer × integer = integer; mixing with float promotes the result to float).

### 6.1 Basic Operations

| Command | Pops | Pushes | Description |
|---|---|---|---|
| `+` | `a b` | `a+b` | Addition |
| `-` | `a b` | `a-b` | Subtraction |
| `*` | `a b` | `a*b` | Multiplication |
| `/` | `a b` | `a/b` | Division |

```bund
3 4 +      // → 7
10 3 -     // → 7
6 7 *      // → 42
84 2 /     // → 42
```

### 6.2 Multiple / Bulk Operations

`*+`, `*-`, `**`, `*/` consume all values currently on the stack and combine them with the respective operation:

```bund
1 2 3 4 *+    // → 10  (1+2+3+4)
2 3 4 **      // → 24  (2*3*4)
```

### 6.3 Workbench Variants

Every arithmetic command has a `.`-suffixed variant that reads one operand from the workbench:

```bund
10 .          // workbench = 10
4 -.          // pops 4 from stack, subtracts workbench value: 4 - 10 = -6
              // (operand order: stack_value op workbench_value)
```

### 6.4 Floating-Point Functions

Unary functions operating on a single `FLOAT` value:

| Command | Operation |
|---|---|
| `float.sqrt` | Square root |
| `float.abs` | Absolute value |
| `float.ceil` | Ceiling |
| `float.floor` | Floor |
| `float.round` | Round to nearest |
| `float.fract` | Fractional part |
| `float.signum` | Sign (−1.0, 0.0, +1.0) |
| `float.sin` | Sine |
| `float.cos` | Cosine |
| `float.tan` | Tangent |
| `float.asin` | Arc sine |
| `float.acos` | Arc cosine |
| `float.atan` | Arc tangent |
| `float.sinh` | Hyperbolic sine |
| `float.cosh` | Hyperbolic cosine |
| `float.tanh` | Hyperbolic tangent |
| `float.cbrt` | Cube root |

```bund
9.0 float.sqrt     // → 3.0
-5.0 float.abs     // → 5.0
float.Pi float.sin // → ~0.0 (sin π ≈ 0)
```

---

## 7. Logic and Comparison

### 7.1 Boolean Values

Booleans are pushed by writing `true` or `false` (which are command names that push a `BOOL` value).

```bund
true     // stack: [ true ]
false    // stack: [ false ]
```

### 7.2 Comparison Operators

All comparison operators pop two values from the stack and push a `BOOL`.

| Command | Meaning |
|---|---|
| `==` | Equal |
| `!=` | Not equal |
| `>` | Greater than |
| `<` | Less than |
| `>=` | Greater than or equal |
| `<=` | Less than or equal |

```bund
42 42 ==    // → true
3 5 >       // → false
10 2 >=     // → true
```

### 7.3 Boolean Logic

| Command | Pops | Pushes | Description |
|---|---|---|---|
| `not` | `a` | `¬a` | Logical NOT |
| `and` | `a b` | `a∧b` | Logical AND |
| `or` | `a b` | `a∨b` | Logical OR |

```bund
true false and    // → false
true false or     // → true
false not         // → true
```

### 7.4 Type Check — `?type`

`?type` pops a type-name string (e.g. `"INTEGER"`) and peeks at the value below it. It pushes `true` if the type matches, `false` otherwise. The value itself remains on the stack.

```bund
42 "INTEGER" ?type    // stack: [ true | 42 ]
"hi" "FLOAT" ?type    // stack: [ false | "hi" ]
```

---

## 8. Control Flow

### 8.1 Conditional Execution — `if`

`if` pops a `LAMBDA` and then a `BOOL` from the stack. If the bool is `true`, the lambda is executed; otherwise it is discarded.

```bund
true  { "yes" println } if    // prints "yes"
false { "no"  println } if    // does nothing
```

`if.false` is the inverse — executes the lambda when the condition is `false`:

```bund
false { "condition was false" println } if.false
```

`if.` and `if.false.` are the workbench variants that read the boolean from the workbench.

### 8.2 Two-Branch Conditional — `ifthenelse`

`ifthenelse` pops three values: an `else_lambda`, a `then_lambda`, and a `BOOL`. The appropriate branch is executed.

```bund
10 5 >                  // push true
{ "greater" println }   // then-lambda
{ "not greater" println } // else-lambda
ifthenelse              // prints "greater"
```

### 8.3 While Loop — `while`

`while` pops a lambda and executes it repeatedly. The lambda must leave a `BOOL` on the stack; execution continues as long as the bool is `true`.

```bund
0
{ dup 5 <
  { dup println 1 + true }
  { false }
  ifthenelse
} while
drop
```

### 8.4 For Loop — `for`

`for` pops a lambda and executes it. If the lambda leaves `true` on the stack, `for` executes it again. If it leaves `false`, the loop ends.

```bund
0
{ dup println
  1 + dup 5 <
} for
```

### 8.5 Do (Execute Once)

`do` pops the top of the stack (which must be a lambda) and executes it exactly once.

```bund
{ "one-shot" println } do
```

### 8.6 Map — applying a lambda to a list

`map` pops a `LAMBDA` and a `LIST`, applies the lambda to each element in order, and pushes the resulting list of return values.

```bund
[ 1 2 3 4 ] { 2 * } map    // → [ 2 4 6 8 ]
```

### 8.7 Times Repetition

`times` pops an `INTEGER` N and a `LAMBDA`, then executes the lambda N times.

```bund
{ "hello" println } 3 times    // prints "hello" three times
```

---

## 9. Lambda Functions

### 9.1 Anonymous Lambdas

A lambda created with `{ … }` is a first-class value pushed onto the stack. It can be stored, passed, and eventually executed.

```bund
{ "I am a lambda" println }    // push lambda onto stack
do                             // execute it once
```

### 9.2 Registering Named Functions

The pattern `:name { body } register` binds a lambda to a name. After registration, the name can be invoked as a command.

```bund
:greet { "Hello!" println } register

greet    // prints "Hello!"
greet    // prints "Hello!" again
```

The atom (`:name`) provides the string name; `register` consumes both the atom and the lambda from the stack.

### 9.3 Aliases

`alias` pops two atoms and creates an alternative name for an existing registered function.

```bund
:double { 2 * } register
:double :times_two alias

4 times_two    // → 8
```

`unregister` removes a named function. `unalias` removes an alias.

### 9.4 Pointers to Functions

Use the backtick prefix to push a reference to a function as a `PTR` value rather than executing it:

```bund
`greet              // push a pointer to the "greet" function
`println            // push a pointer to println
```

Pointers can be stored in lists or maps and later dereferenced and executed.

### 9.5 Recursive Functions

A registered function can call itself by name:

```bund
:countdown {
    dup 0 >
    { dup println 1 - countdown }
    { "done" println }
    ifthenelse
} register

5 countdown    // prints 5 4 3 2 1 done
```

---

## 10. Collections

### 10.1 Lists

A list `[ v1 v2 … vN ]` is an ordered sequence of heterogeneous values. Lists are immutable in the sense that modification operations produce new lists.

```bund
[ 1 2 3 ]          // integer list
[ "a" 2 true ]     // mixed-type list
[ [ 1 2 ] [ 3 4 ] ] // nested list
```

#### Building lists programmatically

`push` appends a value to a list:

```bund
[ 1 2 ] 3 push    // → [ 1 2 3 ]
```

`pull` pops the last element from a list, pushing both the shorter list and the removed element:

```bund
[ 1 2 3 ] pull    // stack: [ 3 | [1 2] ]
```

### 10.2 Dictionaries (Maps)

A `MAP` value is a string-keyed dictionary. Dictionaries are created empty with the `dict` concept and populated with `set`:

```bund
// Create a map by using set operations
"Alice" "name" {}   // conceptual: key "name" → "Alice"
```

Standard dictionary operations:

| Command | Pops | Pushes | Description |
|---|---|---|---|
| `set` | `value key dict` | `dict'` | Store `value` at `key`, return new dict |
| `get` | `key dict` | `value` | Retrieve value at `key` |
| `has_key` | `key dict` | `dict bool` | Check key existence, leave dict on stack |

```bund
// Build a map
"Alice" "name" set
30 "age" set

// Read values
"name" get println    // prints "Alice"
"age" get println     // prints "30"
```

### 10.3 Matrices

A `MATRIX` is a two-dimensional collection of values. It supports the same `map` and element-access operations as lists.

### 10.4 Queues

`QUEUE` and `FIFO` types provide ordered first-in-first-out collections.

---

## 11. String Operations

### 11.1 Case Conversion

| Command | Description |
|---|---|
| `string.upper` | Convert to UPPERCASE |
| `string.lower` | convert to lowercase |
| `string.title` | Convert To Title Case |
| `string.snake` | convert_to_snake_case |
| `string.camel` | convertToCamelCase |

```bund
"hello world" string.upper    // → "HELLO WORLD"
"HELLO WORLD" string.lower    // → "hello world"
"hello world" string.title    // → "Hello World"
```

### 11.2 Concatenation

`concat_with_space` pops a string and a text buffer and concatenates them with a space separator.

### 11.3 Pattern Matching

| Command | Description |
|---|---|
| `string.wildmatch` | Shell-glob (`*`, `?`) pattern match |
| `string.fuzzy_match` | Fuzzy similarity matching |
| `string.distance` | Edit (Levenshtein) distance |
| `string.regex` | Regular expression match |
| `string.regex_matches` | All regex match groups |
| `string.regex_split` | Split string by regex |
| `string.grok` | Grok (logstash) pattern parsing |
| `string.tokenize` | Tokenise string into word list |
| `string.textwrap` | Wrap text to a given width |

### 11.4 Formatting

`format` performs template substitution using the `leon` template syntax. Push the data, then the template string.

```bund
"Alice" "Hello, {name}!" format    // → "Hello, Alice!"
```

---

## 12. Type Conversion

All type-conversion commands have a `.`-suffixed workbench variant.

| Command | Description |
|---|---|
| `convert.to_string` | Convert any value to its string representation |
| `convert.to_int` | Parse or coerce value to `INTEGER` |
| `convert.to_float` | Parse or coerce value to `FLOAT` |
| `convert.to_bool` | Coerce value to `BOOL` |
| `convert.to_list` | Wrap value in a list (or convert collection) |
| `convert.to_dict` | Convert to `MAP` |
| `convert.to_matrix` | Convert to `MATRIX` |
| `convert.to_textbuffer` | Convert to `TEXTBUFFER` |

```bund
42 convert.to_string     // → "42"
"3.14" convert.to_float  // → 3.14
1 convert.to_bool        // → true
```

### 12.1 Type Inspection

`type` peeks at the top value and pushes its type name as a `STRING` (leaves the original value on the stack).

`type.of` peeks and pushes the numeric type discriminant as an `INTEGER`.

```bund
42 type       // stack: [ "INTEGER" | 42 ]
3.14 type.of  // stack: [ 3 | 3.14 ]
```

---

## 13. Time and Timestamps

`time.now` pushes the current time as a `TIME` value (nanoseconds since Unix epoch).

`time.timestamp` pops an integer (milliseconds) and produces a `TIME` value.

Every value carries a `stamp` field set to the creation time in milliseconds. Use `elapsed` (when available) to measure duration since creation.

---

## 14. JSON Integration

`json.from_value` converts the top value to a `JSON`-type value. `json.to_value` converts a `JSON` value back to the native BUND type hierarchy.

```bund
42 json.from_value        // → JSON value containing 42
[ 1 2 3 ] json.from_value // → JSON array [1,2,3]
```

---

## 15. Output and I/O

| Command | Description |
|---|---|
| `print` | Pop and print without newline |
| `println` | Pop and print with newline |
| `print.` | Print from workbench without newline |
| `println.` | Print from workbench with newline |
| `space` | Push a space character `" "` |
| `nl` | Push a newline character |

```bund
"Hello" print
", " print
"world!" println    // prints "Hello, world!"
```

### 15.1 Execute from Stack

`execute` pops the top value and evaluates it:
- If `STRING`, `CALL`, or `PTR`: resolves and calls as a command name
- If `LIST`: unfolds and executes each element in order
- If `MAP`: executes from the map using a key

`execute.` reads from the workbench.

---

## 16. Standard Library Quick Reference

### Arithmetic
`+` `-` `*` `/` `*+` `*-` `**` `*/`  
`+.` `-.` `*.` `/.` `*+.` `*-.` `**.` `*/.`  
`float.sqrt` `float.abs` `float.ceil` `float.floor` `float.round` `float.fract`  
`float.sin` `float.cos` `float.tan` `float.asin` `float.acos` `float.atan`  
`float.sinh` `float.cosh` `float.tanh` `float.cbrt` `float.signum`  
`float.Pi` `float.E` `float.NaN` `float.+Inf` `float.-Inf`

### Stack and Values
`dup` `swap` `.` `len` `car` `cdr` `at` `head` `tail` `clear_stacks` `drop_stacks`

### Logic and Comparison
`==` `!=` `>` `<` `>=` `<=` `not` `and` `or` `?type`

### Control Flow
`if` `if.` `if.false` `if.false.` `ifthenelse`  
`while` `while.` `for` `for.` `do` `do.` `map` `map.` `times`

### Functions and Lambdas
`register` `unregister` `alias` `unalias` `execute` `execute.`

### Collections
`set` `get` `has_key` `push` `pull` `pop`

### Strings
`string.upper` `string.lower` `string.title` `string.snake` `string.camel`  
`string.wildmatch` `string.fuzzy_match` `string.distance` `string.regex`  
`string.regex_matches` `string.regex_split` `string.grok` `string.tokenize`  
`string.textwrap` `format` `concat_with_space`

### Type Conversion
`convert.to_string` `convert.to_int` `convert.to_float` `convert.to_bool`  
`convert.to_list` `convert.to_dict` `convert.to_matrix` `convert.to_textbuffer`  
`type` `type.of`

### Output
`print` `println` `print.` `println.` `space` `nl`

### Time
`time.now` `time.timestamp`

### JSON
`json.from_value` `json.to_value`
