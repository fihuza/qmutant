# qmutant

Mutation testing for QML.

qmutant changes the JavaScript inside your `.qml` files one small edit at a
time (`<` becomes `<=`, `&&` becomes `||`, a handler body becomes `{}`) and
runs your test suite against each change. A change your tests still pass
is a **survivor**: behaviour no test pins down. The share of changes the
tests catch is the **mutation score**.

- Parses QML with a real grammar, so only code is mutated, never comments,
  strings used as keys, imports or declarations
- Runs mutants in parallel, each in its own copy of the project
- Kills a hung test run, including every process it started, when its
  deadline passes
- Fails the run below a score you choose
- Writes a terminal summary, a report in the
  [mutation-testing-report-schema](https://github.com/stryker-mutator/mutation-testing-elements/tree/master/packages/report-schema)
  format as JSON or TOML, and an HTML report

## Inspired by StrykerJS

qmutant is modelled on [StrykerJS](https://stryker-mutator.io), the mutation
testing framework for JavaScript, and brings the same approach to the QML half
of a Qt Quick project. If a project already runs StrykerJS on its `.js`
files, qmutant covers its `.qml` files in the same terms:

| | Compatible with StrykerJS |
|---|---|
| Mutator names | Yes: `EqualityOperator`, `ConditionalExpression`, `StringLiteral` and the rest mean the same change |
| JSON report | Yes: the same [mutation-testing-report-schema](https://github.com/stryker-mutator/mutation-testing-elements/tree/master/packages/report-schema) |
| TOML report | StrykerJS has none; it is the same document as the JSON report |
| HTML report | Yes: the same mutation-testing-elements viewer |
| Mutation score | Yes: (killed + timeout) / (killed + timeout + survived) |
| `high`, `low`, `break` thresholds | Yes: the same meaning |
| Timeout | Yes: `factor × unmutated time + ms` |
| Configuration file | No: `qmutant.toml`, not `stryker.config.json` |
| Disable comments | No: `// qmutant: disable ...`, and a reason is required |

What qmutant does not share is how mutants run. StrykerJS compiles every
mutant into one copy of the code and switches between them at runtime; QML's
declarative syntax cannot express that switch, so qmutant gives each worker
its own copy of the project instead.

## Install

With [mise](https://mise.jdx.dev):

```bash
mise use github:fihuza/qmutant
```

Or download `qmutant-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` from the
[releases](https://github.com/fihuza/qmutant/releases), check it against its
`.sha256`, and put `qmutant` on your `PATH`.

From source, with the Rust version in `rust-toolchain.toml`:

```bash
cargo install --locked --git https://github.com/fihuza/qmutant
```

## Quick start

```bash
qmutant init --command "QT_QPA_PLATFORM=offscreen qmltestrunner -input tests"
qmutant run
```

`init` writes a `qmutant.toml` listing the project's `.qml` files, leaving
out the ones holding its tests -- anything named `tst_*.qml`, and anything
under a `test` or `tests` directory. Mutating a test changes what it asks
rather than what the code does, so a score counting those describes nothing.

`run` first checks that the tests pass unmutated, then runs every mutant and
prints the survivors:

```
Survived  Service.qml:12:33  StringLiteral
  - property string scriptPath: ""
  + property string scriptPath: "qmutant"

┌─────────────┬───────┬────────┬─────────┬──────────┬─────────┬───────┬─────────┐
│ File        ┆ Score ┆ Killed ┆ Timeout ┆ Survived ┆ Invalid ┆ Error ┆ Ignored │
╞═════════════╪═══════╪════════╪═════════╪══════════╪═════════╪═══════╪═════════╡
│ Panel.qml   ┆ 20.45 ┆ 81     ┆ 0       ┆ 315      ┆ 0       ┆ 0     ┆ 0       │
│ Service.qml ┆ 66.47 ┆ 230    ┆ 0       ┆ 116      ┆ 0       ┆ 0     ┆ 0       │
│ All files   ┆ 41.91 ┆ 311    ┆ 0       ┆ 431      ┆ 0       ┆ 0     ┆ 0       │
└─────────────┴───────┴────────┴─────────┴──────────┴─────────┴───────┴─────────┘
Mutation score: 41.91%
```

## Usage

```
qmutant run   [--config FILE] [--mutate GLOB]... [-j N|N%] [--timeout MS]
              [--timeout-factor F] [--reporter NAME]... [--dry-run] [-v]
qmutant list  [--config FILE] [--mutate GLOB]...
qmutant init  --command COMMAND
```

| Command | What it does |
|---|---|
| `run` | Runs the tests against every mutant and reports the score |
| `run --dry-run` | Only checks that the tests pass unmutated and says how many mutants would run |
| `list` | Prints every mutant with its id, position and replacement, running nothing |
| `init` | Writes `qmutant.toml` for the current directory, without the test files |

A flag replaces the value from `qmutant.toml`; it does not add to it.
`-v` logs progress, `-vv` logs every mutant; `RUST_LOG` overrides both.

## Configuration

`qmutant.toml`, in the project root. Only `command` is required.

```toml
mutate = ["**/*.qml"]
command = "QT_QPA_PLATFORM=offscreen qmltestrunner -input tests"
jobs = "50%"
timeout = { ms = 5000, factor = 1.5 }
thresholds = { high = 80, low = 60, break = 100 }
reporters = ["terminal", "progress", "html"]
sandbox_dir = ".qmutant"
ignore = []
exclude_mutators = []
```

| Key | Default | Meaning |
|---|---|---|
| `mutate` | `["**/*.qml"]` | Globs of the files to mutate, relative to the config file. Anything matched must be `.qml`. |
| `command` | — | The command that runs your tests, through `sh -c`, from the project root. Exit 0 means the tests passed. |
| `jobs` | `"50%"` | Parallel workers: a number, or a share of the CPUs. |
| `timeout.ms`, `timeout.factor` | `5000`, `1.5` | A mutant is stopped after `factor × (time of the unmutated run) + ms`. |
| `thresholds.high`, `thresholds.low` | `80`, `60` | Scores at or above `high` show green, at or above `low` yellow, below it red. |
| `thresholds.break` | none | Below this score, `run` exits 1. |
| `reporters` | `["terminal", "progress", "html"]` | Any of `terminal`, `progress`, `json`, `toml`, `html`. |
| `sandbox_dir` | `".qmutant"` | Where the per-worker copies live while a run is going. Removed afterwards. |
| `ignore` | `[]` | Extra gitignore-style patterns not copied into the sandboxes. `.gitignore` is already honoured and `.git` is never copied. |
| `exclude_mutators` | `[]` | Mutators never to apply, by name. |

Unknown keys are an error, so a misspelt key cannot silently do nothing.

### What the test command sees

Each worker has its own full copy of the project, and the command runs inside
it with:

| Variable | Value |
|---|---|
| `QMUTANT_WORKER` | The worker's number, from `0` |
| `QMUTANT_MUTANT` | The id of the mutant under test, as `list` prints it |

The command runs in its own process group. When its deadline passes, or when
you press Ctrl-C, the whole group is killed, so a test runner it started does
not outlive it.

## Mutators

Only the JavaScript inside bindings, functions and signal handlers is
mutated. Imports, pragmas, `id:`, signal and property declarations are
never touched.

| Mutator | Changes |
|---|---|
| `ArithmeticOperator` | `+`↔`-`, `*`↔`/`, `%`→`*` (not string concatenation) |
| `ArrayDeclaration` | `[a, b]` → `[]` |
| `ArrowFunction` | `x => expr` → `x => undefined` |
| `AssignmentOperator` | `+=`↔`-=`, `*=`↔`/=`, `%=`→`*=`, `&&=`↔`\|\|=`, `??=`→`&&=` |
| `BlockStatement` | a non-empty `{ ... }` body → `{}` |
| `BooleanLiteral` | `true`↔`false`, `!x` → `x` |
| `ConditionalExpression` | an `if` or ternary test → `true` and `false`; a loop test → `false` |
| `EqualityOperator` | `<`→`<=`/`>=`, `<=`→`<`/`>`, `>`→`>=`/`<=`, `>=`→`>`/`<`, `==`↔`!=`, `===`↔`!==` |
| `LogicalOperator` | `&&`↔`\|\|`, `??`→`&&` |
| `MethodExpression` | `startsWith`↔`endsWith`, `some`↔`every`, `min`↔`max`, `toUpperCase`↔`toLowerCase`, `trimStart`↔`trimEnd`; `trim`, `slice`, `filter`, `sort`, `reverse`, `substring`, `substr`, `charAt` calls removed |
| `ObjectLiteral` | `{ a: 1 }` → `{}` |
| `OptionalChaining` | `a?.b` → `a.b`, `a?.[i]` → `a[i]`, `f?.()` → `f()` |
| `StringLiteral` | `"text"` → `""`, `""` → `"qmutant"` (not object keys) |
| `UnaryOperator` | `+x`↔`-x` |
| `UpdateOperator` | `++`↔`--` |

## Disabling a mutant

Some mutants cannot be killed because they do not change behaviour. Disable
them where they are, with the reason:

```qml
// qmutant: disable next-line EqualityOperator -- the list never holds more than one entry
visible: entries.length >= 1
```

```qml
// qmutant: disable StringLiteral,ArrayDeclaration -- decoration only
...
// qmutant: enable StringLiteral,ArrayDeclaration
```

- `disable next-line` covers the following line; `disable` covers everything
  after it until an `enable` for the same mutators
- `all` stands for every mutator
- The reason after `--` is required
- A `disable` that no longer disables anything fails the run, so an
  explanation cannot outlive the code it explained

## Statuses and score

| Status | Meaning | Counted |
|---|---|---|
| Killed | The tests failed | detected |
| Timeout | The tests ran past the deadline | detected |
| Survived | The tests passed | undetected |
| Invalid | The mutant does not parse, so it never ran | not counted |
| Error | The command could not be started | not counted |
| Ignored | Disabled by a comment | not counted |

Mutation score = (killed + timeout) / (killed + timeout + survived).

## Exit codes

| Code | Meaning |
|---|---|
| `0` | The score met `thresholds.break`, or no break is set |
| `1` | The score is below `thresholds.break`, or a `disable` comment disables nothing |
| `2` | The configuration is wrong, a directive is malformed, or the tests fail before anything is mutated |
| `130` | Interrupted with Ctrl-C; sandboxes are removed first |

## Reports

| Reporter | Writes |
|---|---|
| `terminal` | Survivors, the score table and the score, to standard output |
| `progress` | A progress bar on standard error while mutants run |
| `json` | `reports/mutation.json` |
| `toml` | `reports/mutation.toml` |
| `html` | `reports/mutation.html` |

`json` and `toml` hold the same document in the
mutation-testing-report-schema format, field for field, so a tool can read
whichever it prefers. The HTML page loads the
[mutation-testing-elements](https://github.com/stryker-mutator/mutation-testing-elements)
viewer from jsDelivr, pinned to a version and checked by its hash, so viewing
it needs a network connection.

## Requirements

- Linux x86_64
- `sh`, and whatever your own test command needs

qmutant itself runs no other external program.

## License

[MIT](LICENSE)
