## Justfile

To run all development checks, install the `just` task runner and:

```sh
just full
```

Requirements:

- Rust toolchain
- `uv` from Astral
- `ty` from Astral
- `echo-comment` (via cargo)

## Precommit

Alternatively, pre-commit bundles all the dependencies for CI and you can just run these and execute
the tests you need to check.

```sh
pre-commit run --all-files
```
