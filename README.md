# ripcloc

Counts lines of code by language, split into source and tests.

```bash
ripcloc               # the current directory
ripcloc engine a.rs   # several files and directories
ripcloc -o json
```

Projects are detected from manifests such as `Cargo.toml`, `go.mod`, `package.json`, `pyproject.toml` and `Gemfile`,
including ones in subdirectories. Each project's conventions decide what is a test and which build and dependency
directories, such as `target/` and `node_modules/`, are skipped. Rust's inline `#[cfg(test)]` modules count as tests.
Workspace members and nested projects are broken out per package. Files ignored by git, and files over 10MB, are not
counted.

`NO_COLOR` disables color, and `COLOR` forces it.
