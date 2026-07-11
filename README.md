# crust

> An experiment: a more C-like syntax on top of Rust, compiled to native binaries.

**What this is:** crust started as an experiment — could I write a more C-like language
by using Rust as the backend, and get familiar with Rust along the way? That is still
what it is. Most of the low-hanging fruit has been reworked to feel like C: functions,
declarations, control flow, structs, pattern matching, threads, and more all have a
C-familiar shape. But turning this into a fully fledged dialect — a real fusion of C
and Rust — involves a lot of hard design decisions that haven't been made yet. Treat
it accordingly: a working transpiler and an exploration, not a finished language.
What the experiment has shown so far is written up in
[First conclusions](#first-conclusions-from-the-experiment) below.

**Status:** the experiment has run its course and crust is kept as-is — a
working transpiler and its write-up. Taking the lessons from it, I am now
investigating a new language designed from scratch: its own semantic model
and a complete backend, rather than a syntax skin over Rust.

The idea: write in a clean, C-like dialect and get Rust's safety, speed, and `cargo`
ecosystem underneath. Source files (`.cru`/`.crust`) are translated to Rust and compiled
to native binaries, so the syntax feels like home if you know C or C++ while the memory
safety guarantees come from the Rust compiler doing the actual work.

You write crust and get a binary; the generated Rust is a backend detail you normally
don't need to read. When you're curious (or when something breaks), `--emit-rs` shows
exactly what the transpiler produced.

## Example

**hello.cru**
```c
function greet(string name): string
{
    return "Hello, " + name + "!";
}

function main()
{
    string msg = greet("world");
    println("{}", msg);
}
```

```
$ crust hello.cru
$ ./hello
Hello, world!
```

Use `--emit-rs` to inspect the Rust backend output:

```
$ crust --emit-rs hello.cru
```
```rust
fn greet(name: String) -> String
{
    return format!("{}{}", format!("{}{}", "Hello, ", name), "!");
}

fn main()
{
    let args: Vec<String> = std::env::args().collect();
    let msg: String = greet(String::from("world"));
    println!("{}", msg);
}
```

(`args` is injected into every `main` — it's how crust programs read their
command line, see below.)

## Installation

**1. Install Rust** (if not already installed)

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

See [rustup.rs](https://rustup.rs) for platform-specific instructions.
This gives you `rustc` (the Rust compiler) and `cargo` (the build tool), both required by crust.

**2. Install crust**

```
cargo install --git https://github.com/bjornwennberg71/crust
```

This builds crust from source and puts the `crust` binary on your PATH.

## Usage

```
crust <file.cru|file.crust>           compile and produce a binary
crust <file> -o <name>                compile with a custom output name
crust --release <file>                optimized build
crust --emit-rs <file>                print generated Rust to stdout, do not compile
```

**Typical workflow:**
```
crust myprogram.cru      # produces ./myprogram
./myprogram
```

**Optimized build:**
```
crust --release myprogram.cru
```

**Inspect generated Rust** — useful for debugging or understanding the backend:
```
crust --emit-rs myprogram.cru
```

**Errors** report the source location with the offending line and a caret:
```
myprogram.cru:4:5: error: expected ';', got identifier 'println'
    println("{}", x);
    ^
```

**Dependencies** — add a `crust.toml` next to your `.cru` file:
```toml
[dependencies]
serde = "1.0"
```

Build artifacts are cached in `.crust/<name>/` next to your source file for incremental
rebuilds. Add `.crust/` to your `.gitignore`.

**Cross-compile and other cargo options** — edit the generated `Cargo.toml` in
`.crust/<name>/` directly, or add `[profile.release]` sections there. It is preserved
between builds.

## Language reference

### Functions
```c
function add(int a, int b): int
{
    return a + b;
}

public function greet()        // public = visible outside this module
{
    // no return type = void
}
```

### Reference parameters
Pass by reference with `&` (read-only) or `&mut` (mutable). Works for any type.

```c
function sum(&Vec<int> v): int   // read-only reference — v is not moved
{
    int total = 0;
    for (x : v)
    {
        total += x;
    }
    return total;
}

function fill(&mut Vec<int> v, int n)   // mutable reference — can modify v
{
    for (i : 0..n)
    {
        v.push(i);
    }
}

mutable Vec<int> nums = vec();
fill(&mut nums, 5);
int s = sum(&nums);
```

### Variables
Const by default, like Rust: assigning to a plain variable is a compile
error. Declare it `mutable` to opt in (`mut` is the accepted short form).

```c
int x          = 42;       // explicit type — read-only
float y        = 3.14;
bool flag      = true;
string s       = "hello";  // owned String
let z          = x + 1;    // inferred type — read-only
auto v         = x * 2;    // `auto` is the same as `let` — C++ spelling
mutable int m  = 0;        // opt in to mutation
auto mutable w = x + 2;    // inferred + mutable
m = m + 1;                 // ok
x = 0;                     // ERROR: x is not mutable
```

Function parameters, range-for loop variables, and `switch` pattern bindings
are always read-only. The counter of a C-style `for (int i = 0; ...)` is
implicitly mutable — the update clause exists to mutate it.

### if / else if / else
```c
if (x > 0)
{
    return x;
}
else if (x == 0)
{
    return 0;
}
else
{
    return 0 - x;
}
```

### if let
```c
if let Some(n) = maybe
{
    int doubled = n * 2;
}
```

### while / while let
```c
while (i < 10)
{
    i = i + 1;
}

while let Some(val) = stack.pop()
{
    process(val);
}
```

### C-style for loop
```c
for (int i = 0; i < n; i++)
{
    total += i;
}
```

### range-for loop (range and collection)
C++ range-for style. `auto` is implied; writing it is fine.
```c
for (i : 0..5)        { ... }    // exclusive: 0, 1, 2, 3, 4
for (i : 1..=5)       { ... }    // inclusive: 1, 2, 3, 4, 5
for (x : items)       { ... }    // iterate over a collection
for (auto x : items)  { ... }    // same — explicit auto
```

Also accepted: `in` instead of `:` (`for (x in items)`), and the bare
Rust-style form without parentheses (`for x in items`).

### Increment / decrement
```c
i++;    i--;    ++i;    --i;    // all desugar to i += 1 / i -= 1
```

### break / continue
```c
while (running)
{
    if (done)
    {
        break;
    }
    if (skip)
    {
        continue;
    }
}
```

> **Note:** `continue` inside a C-style `for` loop skips the update expression.
> Use `continue` in `while` and range-for loops for correct semantics.

### switch
C's `switch`, upgraded: cases are full patterns — constants, alternatives,
and enum/Result destructuring. No fallthrough, ever.

```c
switch (n)
{
    case 1 | 2 | 3:
    {
        return 1;
    }
    case -1:
    {
        return 0 - 1;
    }
    default:
    {
        return 0;
    }
}

switch (fs::read_to_string(path))
{
    case Ok(content): { use_it(content); }     // destructuring — payload
    case Err(e):      { println("{}", e); }    // is bound to a variable
}
```

The Rust spelling `match x { pattern: { ... } }` is also accepted — same
construct, no `case` keyword, `=>` allowed instead of `:`.

### Structs
```c
@derive(Debug, Clone)
public struct Point
{
    public float x;
    public float y;
}

Point p = Point { x: 1.0, y: 2.0 };
```

### impl blocks
```c
impl Point
{
    public static function new(float x, float y): Point   // static — no this
    {
        return Point { x: x, y: y };
    }

    public function length(): float                        // read-only by default
    {
        return this.x * this.x + this.y * this.y;
    }

    public function scale(float factor) mutable            // mutable — can modify this
    {
        this.x *= factor;
        this.y *= factor;
    }
}

mutable Point p = Point::new(3.0, 4.0);
float len       = p.length();
p.scale(2.0);
```

### Enums
```c
@derive(Debug)
public enum Shape
{
    Circle(float),
    Rect(float, float),
    Point,
}

Shape s = Shape::Circle(3.0);

switch (s)
{
    case Shape::Circle(r):   { return r * r; }
    case Shape::Rect(w, h):  { return w * h; }
    case Shape::Point:       { return 0; }
}
```

### Traits
```c
trait Greet
{
    function hello(): string;           // abstract — must implement

    function goodbye(): string          // default implementation
    {
        return "goodbye";
    }
}

impl Greet for Person
{
    public function hello(): string
    {
        return format("Hello, {}!", this.name);
    }
}

// accept any type that implements Greet
function print_greeting(impl Greet item)
{
    println("{}", item.hello());
}
```

### Generics in type position
```c
function find(Vec<int> items, int target): Option<int>
{
    return None;
}

Option<int>       maybe  = Some(42);
Result<int, bool> r      = Ok(1);
Vec<float>        scores = vec();
```

### Tuples
```c
(int, float) pair = (1, 2.0);
int   a = pair.0;
float b = pair.1;
```

### Arrays
```c
int arr[4] = [10, 20, 30, 40];   // fixed size
int arr[]  = [10, 20, 30, 40];   // slice
int first  = arr[0];
```

### Vec methods
```c
mutable Vec<int> v = vec();  // empty; vec(1, 2, 3) creates with elements
v.push(42);
v.pop();
v.remove(i);            // remove element at index i
v.insert(i, 42);        // insert 42 before index i
v.swap(i, j);           // swap elements at i and j
v.sort();
v.reverse();
int  len   = v.len();
bool empty = v.is_empty();

bool has = v.contains(42);   // → v.contains(&42)
bool has = v.contains(x);    // → v.contains(&x)

auto slice = v[1..3];        // slice range
```

### Iterator chains
```c
Vec<int> nums = vec(1, 2, 3, 4, 5);

Vec<int> doubled = nums.iter().map(function(int x) { return x * 2; }).collect();
Vec<int> evens   = nums.iter().filter(function(int x) { return x % 2 == 0; }).collect();
int sum          = nums.iter().sum();
int count        = nums.iter().count();
```

### HashMap
No import needed — `HashMap` is built in, like `Vec`.

```c
mutable HashMap<string, int> scores = HashMap::new();
scores.insert("Alice", 100);
scores.remove("Alice");

bool has = scores.contains_key("Alice");
int  len = scores.len();

if let Some(val) = scores.get("Alice")
{
    println("score: {}", val);
}

for (key   : scores.keys())
{
    println("{}", key);
}
for (val   : scores.values())
{
    println("{}", val);
}
for (entry : scores.iter())
{
    println("{:?}", entry);
}
```

### Strings
```c
string name     = "Alice";
string greeting = "Hello, " + name;   // → format!(...)

int    len   = name.len();
bool   has   = name.contains("li");
string upper = name.to_uppercase();
string lower = name.to_lowercase();
bool   empty = name.is_empty();
```

### Print / format
```c
println("hello {}", name);            // no ! needed — crust adds it
string msg = format("hi, {}!", name);
```

### Closures
```c
auto double  = function(int x) { return x * 2; };
auto is_even = function(int x) { return x % 2 == 0; };
int result   = double(5);
```

### Threads
`spawn(...)` starts a thread from a named function or a closure and returns a
`thread` handle; `join()` waits for it. `sleep_ms(n)` pauses the current thread.

```c
function worker()
{
    println("working");
}

function main()
{
    thread t1 = spawn(worker);              // spawn a named function

    string msg = "captured";
    thread t2 = spawn(function() {          // spawn a closure
        println("thread owns: {}", msg);
    });

    sleep_ms(50);

    t1.join();
    t2.join();
}
```

Values a closure captures are **moved into the thread automatically** — no
keyword needed, but after the `spawn(...)` line the spawning function can no
longer use `msg`. When both sides need the data, share it with `arc<T>` and
give each side a `.clone()`.

### Heap and shared pointers
Plain constructor calls — no `make_shared`-style ceremony.

```c
box<int>    b = box(5);        // heap allocation             (Rust Box<T>)
rc<int>     r = rc(42);        // reference counted, 1 thread (Rust Rc<T>)
arc<string> s = arc("shared"); // atomic ref count, threads   (Rust Arc<T>)

rc<int>     r2 = r.clone();    // second owner of the same allocation
arc<string> s2 = s.clone();    // clone a handle, move it into a thread
```

### async / await
C#-style: `async function` declares, prefix `await` consumes.

```c
async function fetch(): string
{
    return "data";
}

async function main()
{
    string data = await fetch();
    println("{}", data);
}
```

When a program uses `async`, crust adds the tokio runtime to the generated
build automatically — nothing to configure.

### Deriving traits
`@derive(...)` is the canonical form; the Rust-style `#[derive(...)]` attribute
is also accepted.

```c
@derive(Debug, Clone)
struct Point
{
    int x;
    int y;
}

#[derive(Debug, Clone, PartialEq)]
enum Color
{
    Red,
    Green,
    Blue,
}
```

### Operators
```c
counter += 1;           // compound assignment: += -= *= /=
bool flipped = !flag;   // logical not
int  neg     = -42;
int  val     = *ptr;    // dereference
int* ref_    = &x;      // address-of
bool both    = a && b;
bool either  = a || b;
float f      = (float)x;    // C-style cast
float f      = x as float;  // Rust-style cast — both accepted
```

### Type aliases and constants
```c
type Meters  = float;
type Seconds = float;

const int   MAX_SPEED = 300;
static float PI       = 3.14159;
```

### use
`use` imports external libraries — crates.io dependencies declared in
`crust.toml`, or the Rust standard library. Builtin types (`Vec`, `HashMap`,
`string`, `box`/`rc`/`arc`, `thread`) never need an import.

```c
use rumqttc::Client;    // from crust.toml [dependencies]
use std::fs;            // std library, e.g. fs::read_to_string(path)
```

### extern "C"
```c
extern "C"
{
    void printf(*char fmt);
    int  strlen(*char s);
}

function main()
{
    printf("hello\n");      // wrapped in unsafe {} automatically
    int n = strlen("hi");
}
```

### unsafe
```c
unsafe
{
    int y = x * 2;
}
```

### command-line arguments

`args` is automatically available in `main()` as a `Vec<string>` — no import needed.
Same convention as C's `argv`: index 0 is the program name, user arguments
start at index 1 (`args.len()` plays the role of `argc`).

```c
function main()
{
    int n = 10000;          // default
    if (args.len() > 1)
    {
        n = args[1].parse();
    }
}
```

## Examples

Side-by-side C and crust. Each example shows the same program in both languages so
you can see exactly what changes and what stays familiar.

---

### Structs and methods

**C**
```c
#include <math.h>
#include <stdio.h>

typedef struct {
    float x;
    float y;
} Point;

float point_distance(const Point *a, const Point *b)
{
    float dx = a->x - b->x;
    float dy = a->y - b->y;
    return sqrtf(dx*dx + dy*dy);
}

int main(void)
{
    Point a = { 1.0f, 2.0f };
    Point b = { 4.0f, 6.0f };
    printf("distance: %f\n", point_distance(&a, &b));
    return 0;
}
```

**crust**
```c
@derive(Debug)
struct Point
{
    float x;
    float y;
}

impl Point
{
    function distance(&Point other): float
    {
        float dx = this.x - other.x;
        float dy = this.y - other.y;
        return (dx*dx + dy*dy).sqrt();
    }
}

function main()
{
    Point a = Point { x: 1.0, y: 2.0 };
    Point b = Point { x: 4.0, y: 6.0 };
    println("distance: {}", a.distance(&b));
}
```

Methods live inside `impl` blocks attached to the struct. `this` replaces `self`.
No separate header file. No `typedef`. No pointer arithmetic.

---

### Error handling

C returns `NULL` or an error code and hopes the caller checks. Crust uses
`Result<T, E>` — the compiler refuses to let you ignore the error.

**C**
```c
#include <stdio.h>
#include <stdlib.h>

char *read_file(const char *path)
{
    FILE *f = fopen(path, "r");
    if (!f) return NULL;   // caller must check — nothing enforces this

    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    rewind(f);

    char *buf = malloc(len + 1);
    fread(buf, 1, len, f);
    buf[len] = '\0';
    fclose(f);
    return buf;            // caller must free — nothing enforces this
}

int main(void)
{
    char *content = read_file("notes.txt");
    if (!content)
    {
        fprintf(stderr, "failed\n");
        return 1;
    }
    printf("%s", content);
    free(content);
    return 0;
}
```

**crust**
```c
use std::fs;

function read_file(string path): Result<string, string>
{
    return fs::read_to_string(path).map_err(function(e) { return e.to_string(); });
}

function main()
{
    switch (read_file("notes.txt"))
    {
        case Ok(content): { println("{}", content); }
        case Err(e):      { println("error: {}", e); }
    }
}
```

No `malloc`, no `free`, no NULL check. Memory is freed automatically when `content`
goes out of scope.

---

### String splitting and parsing

**C**
```c
#include <stdio.h>
#include <string.h>

int main(void)
{
    char line[] = "alice:30:engineer";
    char *parts[3];
    int   n   = 0;
    char *tok = strtok(line, ":");     // mutates the original string
    while (tok && n < 3)
    {
        parts[n++] = tok;
        tok = strtok(NULL, ":");
    }
    printf("name=%s age=%s role=%s\n", parts[0], parts[1], parts[2]);
    return 0;
}
```

**crust**
```c
function main()
{
    string line         = "alice:30:engineer";
    Vec<string> parts   = line.split(":").collect();
    println("name={} age={} role={}", parts[0], parts[1], parts[2]);
}
```

`strtok` destroys the original string and is not thread-safe. `split` returns an
iterator over borrowed slices — the original string is untouched.

---

### Dynamic arrays

C requires manual `malloc`/`realloc`/`free`. Crust's `Vec<T>` grows automatically
and frees itself.

**C**
```c
#include <stdio.h>
#include <stdlib.h>

int main(void)
{
    int  cap  = 8;
    int  len  = 0;
    int *nums = malloc(cap * sizeof(int));

    for (int i = 0; i < 20; i++)
    {
        if (len == cap)
        {
            cap *= 2;
            nums = realloc(nums, cap * sizeof(int));
        }
        nums[len++] = i * i;
    }

    for (int i = 0; i < len; i++) printf("%d ", nums[i]);
    printf("\n");
    free(nums);
    return 0;
}
```

**crust**
```c
function main()
{
    mutable Vec<int> nums = vec();
    for (int i = 0; i < 20; i++)
    {
        nums.push(i * i);
    }
    for (n : nums)
    {
        print("{} ", n);
    }
    println("");
}
```

---

### Pattern matching

C `switch` only matches integers and falls through by default. Crust `switch`
works on any type, extracts enum payloads, and never falls through.

**C**
```c
#include <stdio.h>

typedef enum { CMD_QUIT, CMD_MOVE, CMD_FIRE } CmdKind;
typedef struct { CmdKind kind; int x, y; } Command;

void handle(Command cmd)
{
    switch (cmd.kind)
    {
        case CMD_QUIT: printf("quitting\n");                      break;
        case CMD_MOVE: printf("move to %d,%d\n", cmd.x, cmd.y);  break;
        case CMD_FIRE: printf("fire!\n");                         break;
    }
}
```

**crust**
```c
enum Command
{
    Quit,
    Move(int, int),
    Fire,
}

function handle(Command cmd)
{
    switch (cmd)
    {
        case Command::Quit:        { println("quitting"); }
        case Command::Move(x, y):  { println("move to {},{}", x, y); }
        case Command::Fire:        { println("fire!"); }
    }
}
```

`Move(x, y)` unpacks the payload directly in the pattern — no struct field access,
no separate variables. The compiler warns if any variant is unhandled.

---

### Word count — complete program

A `wc`-style utility that counts lines, words, and bytes in a file.

**C**
```c
#include <stdio.h>
#include <ctype.h>

int main(int argc, char *argv[])
{
    if (argc < 2)
    {
        fprintf(stderr, "usage: wc <file>\n");
        return 1;
    }

    FILE *f = fopen(argv[1], "r");
    if (!f)
    {
        fprintf(stderr, "cannot open %s\n", argv[1]);
        return 1;
    }

    long lines = 0, words = 0, bytes = 0;
    int  c, in_word = 0;
    while ((c = fgetc(f)) != EOF)
    {
        bytes++;
        if (c == '\n') lines++;
        if (isspace(c))
        {
            in_word = 0;
        }
        else if (!in_word)
        {
            in_word = 1;
            words++;
        }
    }
    fclose(f);
    printf("%ld %ld %ld %s\n", lines, words, bytes, argv[1]);
    return 0;
}
```

**crust**
```c
use std::fs;

function main()
{
    if (args.len() < 2)
    {
        println("usage: wc <file>");
        return;
    }

    string path = args[1].clone();

    switch (fs::read_to_string(&path))
    {
        case Err(e): { println("error: {}", e); return; }
        case Ok(content):
        {
            int lines = content.lines().count();
            int words = content.split_whitespace().count();
            int bytes = content.len();
            println("{} {} {} {}", lines, words, bytes, path);
        }
    }
}
```

`args` is `argv` as a `Vec<string>` built into `main()` — same indexing as C,
with `args.len()` instead of a separate `argc`. The file is read in one call;
counting is iterator chains over the result.

---

### battery2mqtt — a complete real-world program

A battery monitoring daemon that simulates a multi-module LiFePO4 pack and
publishes voltage and SoC readings to MQTT. Shows structs, config file parsing,
a main loop, error handling, and crates.io dependencies all working together.

Full source in `examples/battery2mqtt.c` and `examples/battery2mqtt.cru`.

**Topics published:**
```
battery/0/voltage              →  54.28   (aggregate, average of modules)
battery/0/soc                  →  97.30
battery/0/module/0/voltage     →  54.39
battery/0/module/0/soc         →  98.50
battery/0/module/1/voltage     →  54.33
battery/0/module/1/soc         →  98.00
...
```

**Config file** (`battery2mqtt.conf`):
```
mqtt_host           = localhost
mqtt_port           = 1883
topic_prefix        = battery
num_batteries       = 2
modules_per_battery = 4
interval_ms         = 1000
```

**C** — build with `gcc -O2 -o battery2mqtt battery2mqtt.c -lmosquitto`

Key differences from crust:

| | C | crust |
|---|---|---|
| MQTT library | libmosquitto (system dep) | rumqttc via crust.toml |
| Error handling | NULL check + manual free | `switch` on `Result` |
| String topics | `snprintf` into char buf | `format(...)` |
| Config parsing | `sscanf` + `strcmp` chain | iterator + `switch (key)` |
| Loop timing | `usleep` | `sleep_ms(...)` |
| Event loop drain | pthread | `spawn(...)` |
| Memory | manual `malloc`/`free` | automatic |

**crust** — add a `crust.toml` next to the source file:
```toml
[dependencies]
rumqttc = "0.24"
```
Then build with `crust battery2mqtt.cru`.

---

### pci_probe — real hardware access from userspace

A userspace PCI probe in `examples/pci_probe.cru`: enumerates the PCI bus from
sysfs, parses a device's config space from the raw bytes, and — as root — maps
the WiFi card's BAR0 with `extern "C"` `open`/`mmap` and reads the
`SOC_HW_VERSION` hardware register with a volatile load. The companion to
`tests/ath11k_pci.cru`: the test file simulates the driver's register logic,
this one touches the real device.

```
crust examples/pci_probe.cru
./pci_probe          # bus enumeration + config space (no root needed)
sudo ./pci_probe     # + the BAR0 register read
```

Verified output from the QCNFA765 (root run, 2026-07-11):
```
--- config space (64 bytes readable) ---
  vendor=0x17cb  device=0x1103  command=0x0406  rev=1
  BAR0: 64-bit memory, addr=0x78600000

--- BAR0 register read: SOC_HW_VERSION @ 0x24 ---
  raw=0x00000a20  major=0xa  minor=0x2
  matches the value the kernel driver logged for this card (wcn6855 hw2.1)
```

---

## Editor support

### Emacs

Copy `editors/crust-mode.el` to `~/.emacs.d/` and add to your `.emacs`:

```elisp
(load "~/.emacs.d/crust-mode")
```

Opens `.cru` and `.crust` files in `crust-mode`, derived from `c++-mode` for
Allman-style indentation. Adds keyword, type, constant, and `@derive` highlighting
on top of the standard C/C++ faces.

**If you use `lsp-mode` with a `c++-mode` hook** that calls `(lsp)`, guard it so
crust files are skipped (there is no LSP server for crust):

```elisp
;; in your c++-mode hook, replace (lsp) with:
(unless (derived-mode-p 'crust-mode)
  (lsp))
```

## First conclusions from the experiment

Findings so far, written down honestly — what worked, what didn't, and what it
would take to go further.

**The syntax half of the experiment worked, and cheaply.** The whole
transpiler — lexer, parser, sema, emitter — is about 3,000 lines of Rust, and
it covers functions, structs, enums, traits, closures, threads, async, and
extern "C". That ratio is only possible because rustc does the hard parts:
borrow checking, optimization, and code generation come for free. Leaning on
Rust as a backend was the right call for a one-person experiment.

**The hard part is not syntax — it is semantics.** This is the main finding.
Crust hides Rust's *syntax*, but Rust's ownership model still shows through:
values move into spawned threads, sharing requires `arc` + `clone()`, and
errors carry `Ok`/`Err` ceremony. A C programmer meets the borrow checker on
day one, just wearing different clothes. No amount of surface translation
fixes that; it needs a language with its own semantic model (value types,
reference types, an error model) designed up front — which is a far larger
project than a syntax skin, and the reason this stays an experiment.

**Correctness first, performance later — visibly.** Value-semantics iteration
is currently implemented with a defensive `.clone()` (see the appendix).
Eliding those clones safely needs real type and ownership analysis. The sema
pass now does its first genuine work — it collects the whole-program signature
facts the emitter needs — but inference and ownership analysis remain open.

**Error messages were the weakest point, so that got fixed.** Earlier versions
died with a Rust panic dump on a syntax error. The compiler now reports
`file:line:column`, the offending source line, and a caret:

```
bad.cru:4:5: error: expected ';', got identifier 'println'
    println("{}", x);
    ^
```

**Can crust write a Linux driver? Kernel: no. Userspace: yes — verified on
real hardware.** One assignment behind this experiment was to find out whether
crust could write a Linux driver (`tests/ath11k_pci.cru` is a port of the
ath11k PCI driver's probe/reset/IRQ logic — but with the MMIO stubbed out, it
is a simulation, not an answer). The honest split: a *kernel* driver is
structurally out of reach, and not because of size. Crust emits userspace
Rust — std strings, cargo builds, unwinding panics — while a kernel module
needs `no_std`, the in-tree `kernel` crate abstractions, Kbuild integration,
and fallible allocation. Even hand-written Rust cannot do a WiFi driver
upstream yet; there are no mac80211 bindings. A *userspace* driver, however,
is ordinary systems programming, and `examples/pci_probe.cru` demonstrates it
end-to-end on the very card the simulation mimics: it enumerates
`/sys/bus/pci/devices`, parses the QCNFA765's PCI config space from the raw
binary file, and — run as root — maps BAR0 via `extern "C"` `open`/`mmap` and
reads the `SOC_HW_VERSION` register with a volatile load. That is the same
register the kernel driver reads at probe. Real syscalls, real registers, no
simulation.

**What a "real" version would need.** Turning this into a fully fledged
dialect — an actual fusion of C and Rust rather than C-flavored Rust — means
deciding the memory model, the error model, and nullability as *language*
questions, then building a compiler that owns those semantics instead of
borrowing Rust's. That is where the effort goes next: a new language designed
from scratch, with its own semantics and a complete backend, informed by
everything this experiment surfaced. It continues separately from this
repository.

## Not yet covered

- lifetimes (`'a`) — crust hides them; functions that would need explicit
  lifetime annotations aren't expressible yet
- struct destructuring in patterns (`let Point { x, y } = p;`) — use field
  access; tuples destructure via `.0` / `.1`
- C strings — passing a dynamic `string` to an `extern "C"` function requires
  manually embedding the NUL terminator and casting
  (`format("...\0", x).as_ptr() as *i8`), as `examples/pci_probe.cru` does.
  String *literals* are converted automatically; a proper `cstring` type or
  automatic conversion for `*char` parameters is the missing piece

## Appendix: what the backend emits

Everything below is real `crust --emit-rs` output — shown so you can see what
the transpiler produces, not because you ever need to write it.

### Threads

```c
function main()
{
    string msg = "captured";
    thread t = spawn(function() {
        println("{}", msg);
    });
    t.join();
}
```
```rust
fn main()
{
    let args: Vec<String> = std::env::args().collect();
    let msg: String = String::from("captured");
    let t: std::thread::JoinHandle<()> = std::thread::spawn(move || { println!("{}", msg); });
    t.join().unwrap();
}
```

Note the `move` — crust inserts it for every spawned closure, which is why
captures belong to the thread afterwards.

### Pointers

```c
function main()
{
    box<int>    b  = box(5);
    rc<int>     r  = rc(42);
    arc<string> s  = arc("shared");
    arc<string> s2 = s.clone();
}
```
```rust
fn main()
{
    let args: Vec<String> = std::env::args().collect();
    let b: Box<i64> = Box::new(5);
    let r: std::rc::Rc<i64> = std::rc::Rc::new(42);
    let s: std::sync::Arc<String> = std::sync::Arc::new(String::from("shared"));
    let s2: std::sync::Arc<String> = s.clone();
}
```

### Iterators

```c
function main()
{
    Vec<int> nums    = vec(1, 2, 3, 4, 5);
    Vec<int> doubled = nums.iter().map(function(int x) { return x * 2; }).collect();
    int sum          = nums.iter().sum();
}
```
```rust
fn main()
{
    let args: Vec<String> = std::env::args().collect();
    let nums: Vec<i64> = vec!(1, 2, 3, 4, 5);
    let doubled: Vec<i64> = nums.clone().into_iter().map(|x| { return (x * 2); }).collect();
    let sum: i64 = nums.clone().into_iter().sum();
}
```

crust iteration has value semantics: chains see owned elements, and the source
collection stays usable afterwards. Today that is implemented with a clone —
the semantic pass will elide it where the collection isn't used again.

### async / await

```c
async function fetch(): string
{
    return "data";
}

async function main()
{
    string data = await fetch();
    println("{}", data);
}
```
```rust
async fn fetch() -> String
{
    return String::from("data");
}

#[tokio::main]
async fn main()
{
    let args: Vec<String> = std::env::args().collect();
    let data: String = (fetch()).await;
    println!("{}", data);
}
```

The `#[tokio::main]` attribute is why async needs no setup: crust sees it in
the output and adds the tokio dependency to the generated Cargo.toml.
