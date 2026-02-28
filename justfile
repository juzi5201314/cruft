# Cruft command runner (just)
#
# 常用命令统一用 `just` 管理（硬切换：不要手写 cargo 命令）。
#
# WSL target 规则由 `scripts/cargo.cjs` 统一决定：
# - WSL：`x86_64-pc-windows-gnu`
# - 非 WSL：本机默认 target

set positional-arguments

dev *args:
    bun scripts/cargo.cjs dev {{args}}

test *args:
    bun scripts/cargo.cjs test {{args}}

check *args:
    bun scripts/cargo.cjs check {{args}}

clippy *args:
    bun scripts/cargo.cjs clippy {{args}}

fmt *args:
   bun scripts/cargo.cjs fmt {{args}}
