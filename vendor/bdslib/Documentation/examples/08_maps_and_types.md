# 08_maps_and_types.bund

**File:** `examples/08_maps_and_types.bund`

Map (dictionary) operations and runtime type inspection.

## What it demonstrates

- Creating maps with `{ key value ... }` or `map.new`
- `set` / `get` / `has_key`: map mutation and access
- `type` / `type.of`: runtime type inspection
- `convert.*`: type coercion words

## Key words used

| Word | Effect |
|---|---|
| `map.new` | Push a new empty map |
| `set key` | Pop value and map; push map with key set to value |
| `get key` | Pop map; push the value for key |
| `has_key key` | Pop map; push true if key is present |
| `type` | Push the type name of the top-of-stack value |
| `type.of` | Synonym for `type` |
| `convert.int` | Coerce top of stack to integer |
| `convert.float` | Coerce top of stack to float |
| `convert.string` | Coerce top of stack to string |

## Concepts

Maps are first-class values in BUND, passed on the stack like any other value. `set` is non-destructive — it pops the map and the value, then pushes a new map with the key updated. This functional style means maps can be safely shared across branches.

`type` returns a string like `"Int"`, `"Float"`, `"Text"`, `"Bool"`, `"List"`, `"Map"`. Combined with `==`, it enables type dispatch.

## Example

```
map.new
"cpu" 95.4 set
"host" "srv-01" set
"cpu" get println    => 95.4
```
