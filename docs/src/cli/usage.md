# CLI Usage
If you want to get a quick understanding of the interface, simply type `selene --help`.

```
USAGE:
    selene [FLAGS] [OPTIONS] <files>...
    selene <SUBCOMMAND>

FLAGS:
    -h, --help          Prints help information
    -n, --no-summary    Suppress summary information
    -q, --quiet         Display only the necessary information. Equivalent to --display-style="quiet"
    -V, --version       Prints version information

OPTIONS:
        --color <color>                     [default: auto]  [possible values: Always, Auto, Never]
        --config <config>                  A toml file to configure the behavior of selene [default: selene.toml]
        --display-style <display-style>    Sets the display method [possible values: Json, Rich, Quiet]
        --num-threads <num-threads>        Number of threads to run on, default to the numbers of logical cores on your
                                           system [default: your system's cores]
        --pattern <pattern>              Comma-separated globs to match files to check [default: **/*.lua,**/*.luau]

ARGS:
    <files>...

SUBCOMMANDS:
    generate-roblox-std
    help                   Prints this message or the help of the given subcommand(s)
```

## Basic usage

All unnamed inputs you give to selene will be treated as files to check for.

If you want to check a folder of files: `selene files`

If you just want to check one file: `selene code.lua`

If you want to check multiple files/folders: `selene file1 file2 file3 ...`

If you want to pipe code to selene using stdin: `cat code.lua | selene -`

## Advanced options

**-q**

**--quiet**

Instead of the rich format, only necessary information will be displayed.

```
~# selene code.lua
warning[divide_by_zero]: dividing by zero is not allowed, use math.huge instead

   ┌── code.lua:1:6 ───
   │
 1 │ call(1 / 0)
   │      ^^^^^
   │

Results:
0 errors
1 warnings
0 parse errors

~# selene code.lua -q
code.lua:1:6: warning[divide_by_zero]: dividing by zero is not allowed, use math.huge instead

Results:
0 errors
1 warnings
0 parse errors
```

**--num-threads** *num-threads*

Specifies the number of threads for selene to use. Defaults to however many cores your CPU has. If you type `selene --help`, you can see this number because it will show as the default for you.

**--pattern** *pattern*

Comma-separated [globs](https://en.wikipedia.org/wiki/Glob_(programming)) to match what files selene should check for. For example, if you only wanted to check files that end with `.spec.lua`, you would input `--pattern **/*.spec.lua`. Defaults to `**/*.lua`, meaning "any lua file", or `**/*.lua,**/*.luau` with the roblox feature flag, meaning "any lua/luau file".
