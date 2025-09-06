# Git Hooks (Warn-Only)

These local hooks surface hygiene issues without blocking your commit.

- **pre-commit**
  - Warns on `examples/**/scratch*` files
  - Warns when staged changes exceed ~200 LOC (total or per file)
  - Warns if Rust tests contain `#[ignore]` (use `Justify-Ignore:` note in the commit message)

- **commit-msg**
  - Reminds you to include a `Justify-Ignore:` line when `#[ignore]` is present

Bypass on demand:
```bash
SKIP_GIT_HOOKS=1 git commit -m "..."
```

Customize max LOC:
```bash
GIT_HOOKS_WARN_MAX_LINES=250 git commit -m "..."
```