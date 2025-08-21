## Roadmap

See issue 34 which is the roadmap/prioritisation doc

- https://github.com/lmmx/polars-genson/issues/34

## Precommit

Pre-commit bundles all the dependencies for CI and you can just run these and execute
the tests you need to check.

```sh
pre-commit run --all-files
```

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

To install precommit hooks run `just install-hooks` and to run them use `just run-pc`

## Release

### Python

The release is not fully automated because the wheels are failing and I haven't removed the exit
status. To fully automate, either fix the wheel building (TODO) or pass the argument `"#"` (in
quotes!) to the `ship-wheels` recipe which will effectively comment out the `--exit-status` flag to
`gh run watch` and thus allow the script to proceed even when the last job 'failed' (one or more
wheel building failed: i.e. x86 Linux/x86 macOS)

The release process is therefore:

```sh
just release
just ship-wheels "#"
```

- For a non-patch (micro) bump pass a string like "minor"/"major" to the `release` recipe.
- You must not push anything that will trigger CI in the meantime or else the watch (which looks at
  the 0'th job) will look at that instead and potentially upload no wheels/wrong wheels!
    - (This means you cannot push to master as a regular commit will skip CI and get no wheels)

### Rust

The Rust release process is two more commands (if it works the first time it could be one)

```sh
just ship-rust
```

If the dry run fails, you can revert and re-run the last step when it succeeds (but if all is OK you
won't need to):

```sh
just publish-rust
```
