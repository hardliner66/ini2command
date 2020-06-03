# ini2command

## Command Line
```bash
ini2command 1.0
Create a command from a value of an INI-file and execute it.

USAGE:
    ini2command [FLAGS] [OPTIONS] --ini <FILE> --property <NAME> [--] <COMMAND>...

FLAGS:
    -d, --dry        Print the command instead of executing it.
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -i, --ini <FILE>                The path to the ini file
    -p, --property <NAME>           The property to use
    -r, --search_string <STRING>    The search string for use in the template [default: {}]
    -s, --section <NAME>            The section to use

ARGS:
    <COMMAND>...    The command template
```

## Examples

conf.ini:
```ini
ip=8.8.8.8

[addresses]
server1=1.1.1.1
```


For global properties:
```bash
ini2command -i conf.ini -p ip -- ping {}
```

For properties within sections:
```bash
ini2command -i conf.ini -s addresses -p server1 -- ping {}
```

If `{}` cannot be used for some reason:
```bash
ini2command -i conf.ini -p ip -r @@ -- ping @@
```