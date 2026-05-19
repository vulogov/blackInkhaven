# BUND Basic Library Reference

Complete reference for every word (built-in command) provided by the BUND standard library. Each entry lists the exact word name, its stack effect, and a description.

**Stack-effect notation:** `( before -- after )` where the top of the stack is on the right.  
`W:x` denotes a value read from or written to the **workbench** instead of the stack.  
`...` means zero or more additional values consumed or produced.

---

## Table of Contents

1. [Output and Printing](#1-output-and-printing)
2. [Value Creation](#2-value-creation)
3. [Arithmetic Operators](#3-arithmetic-operators)
4. [Float Math Functions](#4-float-math-functions)
5. [Float Constants](#5-float-constants)
6. [Comparison Operators](#6-comparison-operators)
7. [Logic Operators](#7-logic-operators)
8. [Control Flow — Conditionals](#8-control-flow--conditionals)
9. [Control Flow — Loops](#9-control-flow--loops)
10. [Stack and Value Manipulation](#10-stack-and-value-manipulation)
11. [List and Sequence Operations](#11-list-and-sequence-operations)
12. [Dictionary (Map) Operations](#12-dictionary-map-operations)
13. [Type Inspection and Conversion](#13-type-inspection-and-conversion)
14. [String Operations](#14-string-operations)
15. [Time Operations](#15-time-operations)
16. [JSON Operations](#16-json-operations)
17. [Lambda and Function Management](#17-lambda-and-function-management)
18. [Variable Management](#18-variable-management)
19. [Reflection and Introspection](#19-reflection-and-introspection)
20. [File System Operations](#20-file-system-operations)
21. [Console and Terminal](#21-console-and-terminal)
22. [BUND Evaluation](#22-bund-evaluation)

---

## 1. Output and Printing

| Word | Stack effect | Description |
|---|---|---|
| `print` | `( v -- )` | Pop the top value, convert to string, print without a trailing newline. |
| `print.` | `( W:v -- )` | Workbench variant of `print`. |
| `println` | `( v -- )` | Pop the top value, convert to string, print with a trailing newline. |
| `println.` | `( W:v -- )` | Workbench variant of `println`. |
| `space` | `( -- )` | Emit a single space character (no stack effect). |
| `nl` | `( -- )` | Emit a newline character (no stack effect). |

```bund
"Hello" print ", " print "world!" println   // Hello, world!
42 .  println.                              // prints 42 from workbench
```

---

## 2. Value Creation

Words that push ready-made values onto the stack.

| Word | Stack effect | Description |
|---|---|---|
| `true` | `( -- bool )` | Push boolean `true`. |
| `false` | `( -- bool )` | Push boolean `false`. |
| `nodata` | `( -- nodata )` | Push a `NODATA` sentinel value. |
| `list` | `( -- list )` | Push an empty `LIST`. |
| `lambda` | `( -- lambda )` | Push an empty `LAMBDA`. |
| `dict` | `( -- map )` | Push an empty `MAP` (string-keyed dictionary). |
| `valuemap` | `( -- valuemap )` | Push an empty `VALUEMAP` (arbitrary-key dictionary). |
| `text` | `( -- textbuffer )` | Push an empty `TEXTBUFFER`. |
| `metrics` | `( -- metrics )` | Push an empty `METRICS` collection. |
| `class` | `( -- class )` | Push an empty `CLASS` definition. |
| `conditional` | `( -- conditional )` | Push a `CONDITIONAL` through-type value. |
| `pair` | `( a b -- pair )` | Pop two values and push them as an ordered `PAIR`. |
| `complex` | `( real imag -- cfloat )` | Pop real and imaginary parts, push a `CFLOAT` complex number. |
| `ptr` | `( name -- ptr )` | Pop a string name, push a `PTR` reference to the named value. |
| `json` | `( str -- json )` | Pop a JSON string, parse it, push a `JSON` value. |
| `object` | `( name -- obj )` | Pop a class name string, push a new `OBJECT` instance of that class. |

```bund
true println          // true
pair println          // (42, "hello")
"[1,2,3]" json type println   // JSON
```

---

## 3. Arithmetic Operators

Binary operators consume two numeric values and push one result. When one operand is a `FLOAT`, the result is promoted to `FLOAT`; two `INTEGER` operands produce an `INTEGER`.

### 3.1 Basic Binary Operators

| Word | Stack effect | Description |
|---|---|---|
| `+` | `( a b -- a+b )` | Addition. |
| `+.` | `( a W:b -- a+b )` | Addition; second operand from workbench. |
| `-` | `( a b -- a-b )` | Subtraction. |
| `-.` | `( a W:b -- a-b )` | Subtraction; second operand from workbench. |
| `*` | `( a b -- a*b )` | Multiplication. |
| `*.` | `( a W:b -- a*b )` | Multiplication; second operand from workbench. |
| `/` | `( a b -- a/b )` | Division. Integer division when both operands are integers. |
| `/.` | `( a W:b -- a/b )` | Division; second operand from workbench. |

```bund
6 7 *    println   // 42
84 2 /   println   // 42
10 .  4 +. println // 14  (10 workbench + 4 stack)
```

### 3.2 Bulk (Variadic) Operators

These operators consume **all** values currently on the stack and fold them with the operation, pushing one result.

| Word | Stack effect | Description |
|---|---|---|
| `*+` | `( a b ... -- sum )` | Sum of all stack values. |
| `*+.` | `( W:a b ... -- sum )` | Includes workbench value in the sum. |
| `*-` | `( a b ... -- diff )` | Left-fold subtraction of all stack values. |
| `*-.` | `( W:a b ... -- diff )` | Includes workbench value. |
| `**` | `( a b ... -- prod )` | Product of all stack values. |
| `**.` | `( W:a b ... -- prod )` | Includes workbench value. |
| `*/` | `( a b ... -- quot )` | Left-fold division of all stack values. |
| `*/.` | `( W:a b ... -- quot )` | Includes workbench value. |

```bund
1 2 3 4 5 *+   println   // 15
2 3 4 **       println   // 24
```

---

## 4. Float Math Functions

Unary functions that pop one `FLOAT` value and push one `FLOAT` result.

| Word | Stack effect | Description |
|---|---|---|
| `float.sqrt` | `( f -- f )` | Square root. |
| `float.abs` | `( f -- f )` | Absolute value. |
| `float.ceil` | `( f -- f )` | Ceiling (smallest integer ≥ f). |
| `float.floor` | `( f -- f )` | Floor (largest integer ≤ f). |
| `float.round` | `( f -- f )` | Round to nearest integer. |
| `float.fract` | `( f -- f )` | Fractional part (f − floor(f)). |
| `float.signum` | `( f -- f )` | Sign: −1.0, 0.0, or +1.0. |
| `float.cbrt` | `( f -- f )` | Cube root. |
| `float.sin` | `( f -- f )` | Sine (radians). |
| `float.cos` | `( f -- f )` | Cosine (radians). |
| `float.tan` | `( f -- f )` | Tangent (radians). |
| `float.asin` | `( f -- f )` | Arc sine (result in radians). |
| `float.acos` | `( f -- f )` | Arc cosine (result in radians). |
| `float.atan` | `( f -- f )` | Arc tangent (result in radians). |
| `float.sinh` | `( f -- f )` | Hyperbolic sine. |
| `float.cosh` | `( f -- f )` | Hyperbolic cosine. |
| `float.tanh` | `( f -- f )` | Hyperbolic tangent. |

```bund
9.0 float.sqrt    println   // 3.0
-7.5 float.abs    println   // 7.5
float.Pi float.sin println  // ~0.0
2.7 float.floor   println   // 2.0
```

---

## 5. Float Constants

Words that push a constant `FLOAT` value with no arguments.

| Word | Pushes | Description |
|---|---|---|
| `float.Pi` | π ≈ 3.14159… | The mathematical constant π. |
| `float.E` | e ≈ 2.71828… | Euler's number. |
| `float.NaN` | NaN | Not-a-Number. |
| `float.+Inf` | +∞ | Positive infinity. |
| `float.-Inf` | −∞ | Negative infinity. |

```bund
float.Pi 2.0 * println     // ~6.283... (2π)
float.E float.sqrt println // ~1.6487...
```

---

## 6. Comparison Operators

All comparison operators pop two values and push one `BOOL`.

| Word | Stack effect | Description |
|---|---|---|
| `==` | `( a b -- bool )` | Equal. |
| `!=` | `( a b -- bool )` | Not equal. |
| `>` | `( a b -- bool )` | `a` greater than `b`. |
| `<` | `( a b -- bool )` | `a` less than `b`. |
| `>=` | `( a b -- bool )` | `a` greater than or equal to `b`. |
| `<=` | `( a b -- bool )` | `a` less than or equal to `b`. |

```bund
10 5 >    println   // true
3 3 ==    println   // true
7 10 !=   println   // true
```

---

## 7. Logic Operators

| Word | Stack effect | Description |
|---|---|---|
| `not` | `( bool -- bool )` | Logical NOT. |
| `and` | `( a b -- bool )` | Logical AND. |
| `or` | `( a b -- bool )` | Logical OR. |

```bund
true false and   println   // false
true false or    println   // true
false not        println   // true
```

---

## 8. Control Flow — Conditionals

### `if`

```
( bool lambda -- )
```

Pops a `LAMBDA` then a `BOOL`. Executes the lambda if and only if the bool is `true`.

```bund
42 0 > { "positive" println } if
```

### `if.`

```
( lambda W:bool -- )
```

Workbench variant of `if`: the condition is read from the workbench.

### `if.false`

```
( bool lambda -- )
```

Pops a `LAMBDA` then a `BOOL`. Executes the lambda if and only if the bool is `false`.

```bund
0 0 > { "this won't run" println } if.false
```

### `if.false.`

```
( lambda W:bool -- )
```

Workbench variant of `if.false`.

### `if.stack`

```
( stack_name lambda -- )
```

Executes the lambda if the current active stack's name matches `stack_name`.

### `ifthenelse`

```
( bool then_lambda else_lambda -- )
```

Pops `else_lambda`, `then_lambda`, and `bool`. Executes `then_lambda` when true, `else_lambda` when false.

```bund
5 3 >
{ "five is greater" println }
{ "three is greater" println }
ifthenelse
```

### `ifthenelse.`

```
( then_lambda else_lambda W:bool -- )
```

Workbench variant of `ifthenelse`.

### `notifthenelse`

```
( bool then_lambda else_lambda -- )
```

Inverted `ifthenelse`: executes `then_lambda` when the condition is **false**.

### `notifthenelse.`

Workbench variant of `notifthenelse`.

---

## 9. Control Flow — Loops

### `loop`

```
( list lambda -- )
```

Pops a `LAMBDA` and a `LIST`. Executes the lambda once for each element of the list, pushing the element onto the stack before each invocation.

```bund
[ 10 20 30 ] { println } loop   // prints 10, 20, 30
```

### `loop.`

```
( lambda W:list -- )
```

Workbench variant of `loop`.

### `*loop`

```
( lambda -- )
```

Executes the lambda repeatedly, consuming one value from the stack per iteration, until the stack is empty or `NODATA` is encountered.

### `map`

```
( list lambda -- list' )
```

Pops a `LAMBDA` and a `LIST` (or `MATRIX`). Applies the lambda to each element and collects results into a new list.

```bund
[ 1 2 3 4 ] { 2 * } map println   // [ 2 4 6 8 ]
```

### `map.`

```
( lambda W:list -- list' )
```

Workbench variant of `map`.

### `while`

```
( lambda -- )
```

Pops a `LAMBDA` and executes it. After each execution the lambda must leave a `BOOL` on the stack: `true` continues the loop, `false` stops it.

```bund
0
{ dup 5 < { dup println 1 + true } { false } ifthenelse }
while
```

### `while.`

Workbench variant of `while`.

### `for`

```
( lambda -- )
```

Executes the lambda. The lambda's own return value (a `BOOL`) determines whether the loop repeats: `true` → repeat, `false` → stop.

### `for.`

Workbench variant of `for`.

### `do`

```
( lambda -- )
```

Pops and executes the lambda exactly once, draining values from the stack until it is empty.

### `do.`

Workbench variant of `do`.

### `times`

```
( lambda n -- )
```

Pops an `INTEGER` n and a `LAMBDA`. Executes the lambda n times, pushing the iteration index (0, 1, … n−1) onto the stack before each invocation.

```bund
{ println } 3 times   // prints 0, 1, 2
```

### `times.`

```
( lambda W:n -- )
```

Workbench variant of `times`.

---

## 10. Stack and Value Manipulation

| Word | Stack effect | Description |
|---|---|---|
| `.` | `( v -- W:v )` | Pop the top value and move it to the workbench. |
| `+.` | `( W:v -- v )` | Move the workbench value back onto the stack. |
| `dup` | `( v -- v v )` | Duplicate the top value. |
| `swap` | `( a b -- b a )` | Exchange the top two values. |
| `len` | `( v -- v n )` | Peek at the top value; push its length as an `INTEGER`. Does not consume the value. |
| `clear_stacks` | `( ... -- )` | Remove all values from the current stack. |
| `drop_stacks` | `( -- )` | Pop the current stack frame from the stack ring. |
| `execute` | `( v -- ... )` | Execute the top value: resolves `STRING`/`CALL`/`PTR` as a command name; unfolds and executes `LIST` elements in order; dispatches on key for `MAP`. |
| `execute.` | `( W:v -- ... )` | Workbench variant of `execute`. |
| `apply` | `( v -- ... )` | Apply a value to the VM (identical to feeding it as source token). |
| `?move` | `( name bool lambda -- )` | Conditionally move a value to the named stack. Executes `lambda` if condition is true to obtain the value; pushes it onto stack `name`. |
| `?.` | `( val bool -- W:val \| )` | Conditionally move the value to the workbench if `bool` is true. |
| `attribute` | `( attr v -- v' )` | Append `attr` to the attribute list of `v`. |
| `tag` | `( tag_val key v -- v' )` | Add string tag `key → tag_val` to the metadata of `v`. |

```bund
7 dup *    println   // 49  (7*7)
1 2 swap - println   // 1   (2-1 after swap)
42 .  +. println     // 42  (move to workbench and back)
```

---

## 11. List and Sequence Operations

| Word | Stack effect | Description |
|---|---|---|
| `car` | `( list -- first )` | Pop a list; push its first element. |
| `car.` | `( W:list -- first )` | Workbench variant. |
| `cdr` | `( list -- rest )` | Pop a list; push a new list of all elements except the first. |
| `cdr.` | `( W:list -- rest )` | Workbench variant. |
| `head` | `( list n -- list' )` | Pop integer n and list; push new list of the first n elements. |
| `head.` | `( W:list n -- list' )` | Workbench variant. |
| `tail` | `( list n -- list' )` | Pop integer n and list; push new list of the last n elements. |
| `tail.` | `( W:list n -- list' )` | Workbench variant. |
| `at` | `( list n -- elem )` | Pop integer index n and list; push the element at position n (zero-based). |
| `at.` | `( W:list n -- elem )` | Workbench variant. |
| `push` | `( list v -- list' )` | Append value v to the end of the list. |
| `pull` | `( list -- list' last )` | Remove and return the last element of the list. |
| `pop` | `( list -- list' first )` | Remove and return the first element of the list. |

```bund
[ 10 20 30 ] car     println   // 10
[ 10 20 30 ] cdr     println   // [ 20 30 ]
[ 1 2 3 4 5 ] 3 head println   // [ 1 2 3 ]
[ "a" "b" "c" ] 1 at println   // "b"
[ 1 2 ] 3 push       println   // [ 1 2 3 ]
```

---

## 12. Dictionary (Map) Operations

| Word | Stack effect | Description |
|---|---|---|
| `set` | `( dict key val -- dict' )` | Store `val` at `key` in the dictionary; push updated dictionary. |
| `get` | `( dict key -- val )` | Retrieve the value associated with `key` from the dictionary. |
| `has_key` | `( dict key -- dict bool )` | Check whether `key` exists; push the original dictionary and a `BOOL`. |
| `?key` | `( dict key -- dict bool )` | Alias for `has_key`. |

```bund
"Alice" "name" set
30      "age"  set
"name" get println   // Alice
"age"  get println   // 30
"email" has_key { "has email" println } if.false
```

---

## 13. Type Inspection and Conversion

### Inspection

| Word | Stack effect | Description |
|---|---|---|
| `type` | `( v -- v str )` | Peek at the top value; push its type name as a `STRING`. Value is not consumed. |
| `type.of` | `( v -- v n )` | Peek at the top value; push its numeric type discriminant as an `INTEGER`. Value is not consumed. |
| `?type` | `( val name -- bool )` | Pop a type-name `STRING` and a value; push `true` if the value's type matches `name`. The value is consumed. |

```bund
42     type    println   // INTEGER
3.14   type.of println   // 3
"hello" "STRING" ?type   println   // true
```

### Conversion

All conversion words have a workbench variant with a `.` suffix.

| Word | Stack effect | Description |
|---|---|---|
| `convert.to_string` | `( v -- str )` | Convert any value to its string representation. |
| `convert.to_string.` | `( W:v -- str )` | Workbench variant. |
| `convert.to_int` | `( v -- int )` | Parse or coerce value to `INTEGER`. |
| `convert.to_int.` | `( W:v -- int )` | Workbench variant. |
| `convert.to_float` | `( v -- float )` | Parse or coerce value to `FLOAT`. |
| `convert.to_float.` | `( W:v -- float )` | Workbench variant. |
| `convert.to_bool` | `( v -- bool )` | Coerce value to `BOOL` (0 and empty → false). |
| `convert.to_bool.` | `( W:v -- bool )` | Workbench variant. |
| `convert.to_list` | `( v -- list )` | Wrap value in a list, or convert a compatible collection. |
| `convert.to_list.` | `( W:v -- list )` | Workbench variant. |
| `convert.to_dict` | `( v -- map )` | Convert value to a `MAP`. |
| `convert.to_dict.` | `( W:v -- map )` | Workbench variant. |
| `convert.to_matrix` | `( v -- matrix )` | Convert value to a `MATRIX`. |
| `convert.to_matrix.` | `( W:v -- matrix )` | Workbench variant. |
| `convert.to_textbuffer` | `( v -- textbuf )` | Convert value to a `TEXTBUFFER`. |
| `convert.to_textbuffer.` | `( W:v -- textbuf )` | Workbench variant. |

```bund
42      convert.to_string  println   // "42"
"3.14"  convert.to_float   println   // 3.14
0       convert.to_bool    println   // false
```

---

## 14. String Operations

### Case Conversion

All pop one `STRING` and push one `STRING`.

| Word | Description |
|---|---|
| `string.upper` | Convert to `UPPERCASE`. |
| `string.lower` | Convert to `lowercase`. |
| `string.title` | Convert to `Title Case`. |
| `string.snake` | Convert to `snake_case`. |
| `string.camel` | Convert to `camelCase`. |

```bund
"hello world" string.upper   println   // HELLO WORLD
"CamelCase"   string.snake   println   // camel_case
```

### Concatenation

| Word | Stack effect | Description |
|---|---|---|
| `concat_with_space` | `( textbuf str -- textbuf' )` | Append `str` to `textbuf` with a space separator. |
| `format` | `( data... template -- str )` | Template substitution using leon-style `{key}` placeholders. |
| `format.` | `( data... W:template -- str )` | Workbench variant. |

### Pattern Matching and Processing

| Word | Stack effect | Description |
|---|---|---|
| `string.wildmatch` | `( str pattern -- bool )` | Shell-glob match (`*` any sequence, `?` single character). |
| `string.fuzzy_match` | `( str pattern -- bool )` | Fuzzy/approximate string match. |
| `string.distance` | `( a b -- int )` | Edit distance (Levenshtein) between two strings. |
| `string.regex` | `( str pattern -- bool )` | Regular-expression match. |
| `string.regex_matches` | `( str pattern -- list )` | All capture groups from the regex match as a list of strings. |
| `string.regex_split` | `( str pattern -- list )` | Split string on regex pattern; push list of substrings. |
| `string.grok` | `( str pattern -- map )` | Parse string with a grok pattern; push a `MAP` of named captures. |
| `string.tokenize` | `( str -- list )` | Split string into a list of whitespace-delimited tokens. |
| `string.textwrap` | `( str width -- str )` | Wrap text to the given column width. |
| `string.unicode` | `( str -- str )` | Normalize Unicode representation. |
| `string.any_id` | `( -- str )` | Push a random unique identifier string. |
| `string.random` | `( n -- str )` | Push a random string of length n. |
| `string.prefix_suffix` | `( str prefix suffix -- str )` | Wrap string with prefix and suffix. |

```bund
"server.cpu.load" "server.*" string.wildmatch { "matched" println } if
"the quick brown" string.tokenize println   // [ "the" "quick" "brown" ]
"hello@test.com" "[a-z]+@[a-z]+\\.[a-z]+" string.regex { "email" println } if
```

---

## 15. Time Operations

| Word | Stack effect | Description |
|---|---|---|
| `time.now` | `( -- time )` | Push the current timestamp as a `TIME` value (nanosecond resolution). |
| `time.timestamp` | `( ms -- time )` | Pop an integer (milliseconds since epoch) and push a `TIME` value. |

```bund
time.now println   // current timestamp
```

---

## 16. JSON Operations

| Word | Stack effect | Description |
|---|---|---|
| `json` | `( str -- json )` | Parse a JSON string and push a `JSON` value. |
| `json.from_value` | `( val -- json )` | Convert any BUND value to its `JSON` representation. |
| `json.to_value` | `( json -- val )` | Convert a `JSON` value back to the native BUND type hierarchy. |
| `json.path` | `( json path -- val )` | Query the JSON value with a path expression; push the result. |

```bund
"[1,2,3]" json json.to_value println    // [ 1 2 3 ]
42 json.from_value type println          // JSON
```

---

## 17. Lambda and Function Management

| Word | Stack effect | Description |
|---|---|---|
| `register` | `( lambda name -- )` | Bind `lambda` to `name`; after this, `name` is a callable word. Also accepts a `CLASS` value. |
| `unregister` | `( name -- )` | Remove the registered word `name`. |
| `alias` | `( real_name alias_name -- )` | Create `alias_name` as an alternative name for the registered word `real_name`. |
| `unalias` | `( alias_name -- )` | Remove the alias `alias_name`. |
| `resolve` | `( name -- ptr )` | Resolve the name to a `PTR` pointing to the underlying lambda value. |
| `resolve.class` | `( name -- class )` | Resolve a class name to its `CLASS` value. |
| `lambda!` | `( list -- lambda )` | Convert a `LIST` of values to a `LAMBDA`. |
| `lambda*` | `( v... -- lambda )` | Fold all values currently on the stack into a single `LAMBDA`. |

```bund
:square { dup * } register
5 square println           // 25

:square :sq alias
6 sq println               // 36

:square resolve type println  // PTR
```

---

## 18. Variable Management

Variables provide a named, persistent storage outside of any stack.

| Word | Stack effect | Description |
|---|---|---|
| `var` | `( val name -- )` | Store `val` under variable `name`. |
| `var-` | `( name -- )` | Delete the variable `name`. |
| `var?` | `( name -- val )` | Retrieve the value of variable `name`. |

```bund
42 :answer var
:answer var? println   // 42
:answer var-
```

---

## 19. Reflection and Introspection

Words for querying the runtime environment about registered words, aliases, and lambdas.

| Word | Stack effect | Description |
|---|---|---|
| `?alias` | `( name -- bool )` | True if `name` is a registered alias. |
| `?lambda` | `( name -- bool )` | True if `name` is a registered lambda. |
| `?stdlib` | `( name -- bool )` | True if `name` is a built-in standard-library word. |
| `?word` | `( name -- bool )` | True if `name` is any callable word (alias, lambda, or stdlib). |
| `alias=` | `( alias_name -- target_name )` | Push the name that `alias_name` resolves to. |
| `lambda=` | `( name -- lambda )` | Push the lambda value registered under `name`. |

```bund
"println" ?stdlib println   // true
"square"  ?lambda println   // true (if registered above)
"square"  lambda= type println  // LAMBDA
```

---

## 20. File System Operations

| Word | Stack effect | Description |
|---|---|---|
| `file` | `( path -- str )` | Read the entire file at `path` and push its contents as a `STRING`. |
| `file.` | `( W:path -- str )` | Workbench variant of `file`. |
| `filename` | `( path -- path )` | Resolve `path` to its canonical absolute form. |
| `filename.` | `( W:path -- path )` | Workbench variant. |
| `fs.is_file` | `( path -- bool )` | True if `path` refers to an existing regular file. |
| `fs.cwd` | `( -- str )` | Push the current working directory as a string. |
| `fs.cp` | `( src dst -- bool )` | Copy file or directory from `src` to `dst`; push success bool. |
| `fs.mv` | `( src dst -- bool )` | Move (rename) `src` to `dst`; push success bool. |
| `fs.rm` | `( path -- bool )` | Remove file or directory at `path`; push success bool. |
| `fs.ls` | `( path -- list )` | List all entries in the directory at `path`; push a list of name strings. |
| `fs.ls.` | `( W:path -- list )` | Workbench variant. |
| `fs.ls.dir` | `( path -- list )` | List only subdirectory entries. |
| `fs.ls.dir.` | `( W:path -- list )` | Workbench variant. |
| `fs.ls.files` | `( path -- list )` | List only file entries. |
| `fs.ls.files.` | `( W:path -- list )` | Workbench variant. |
| `url` | `( url -- str )` | Fetch the content at `url` and push as a `STRING`. |
| `url.` | `( W:url -- str )` | Workbench variant. |
| `use` | `( uri -- ... )` | Load a BUND source file or URL and execute it. |
| `use.` | `( W:uri -- ... )` | Workbench variant. |

```bund
"/etc/hostname" file println          // contents of /etc/hostname
"." fs.ls println                     // list of current directory entries
"/tmp/test.txt" fs.is_file println    // true or false
```

---

## 21. Console and Terminal

| Word | Stack effect | Description |
|---|---|---|
| `console.clear` | `( -- )` | Clear the terminal screen. |
| `console.title` | `( title -- )` | Set the terminal window title. |
| `console.typewriter` | `( str -- )` | Print `str` with an animated typewriter effect. |
| `console.box` | `( str -- str )` | Format `str` inside a drawn box; push the result string. |

```bund
"My Program" console.title
"Loading..." console.typewriter
"Done" console.box println
```

---

## 22. BUND Evaluation

Words for loading, compiling, and executing BUND code at runtime.

| Word | Stack effect | Description |
|---|---|---|
| `compile` | `( str -- list )` | Parse BUND source `str` and push the resulting word list as a `LIST`. |
| `bund.eval` | `( str -- ... )` | Parse and immediately execute the BUND source string. |
| `bund.eval.` | `( W:str -- ... )` | Workbench variant. |
| `bund.eval-file` | `( path -- ... )` | Load the file at `path` and execute its BUND content. |
| `bund.eval-file.` | `( W:path -- ... )` | Workbench variant. |
| `bund.exit` | `( [code] -- )` | Exit the BUND process. If an integer is on the stack, use it as the exit code; otherwise exit with 0. |

```bund
"42 println" bund.eval              // evaluates and prints 42
"examples/01_hello_world.bund" bund.eval-file
```
