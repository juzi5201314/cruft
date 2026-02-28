#!/usr/bin/env bun
'use strict';

const fs = require('node:fs');
const { spawnSync } = require('node:child_process');

function usage() {
  // 保持输出简洁，避免引入额外文档依赖
  // 用法：
  //   bun scripts/cargo.cjs <dev|test|check|clippy|fmt> [args...]
  process.stderr.write(
    'Usage: bun scripts/cargo.cjs <dev|test|check|clippy|fmt> [args...]\n'
  );
  process.exit(2);
}

function isWsl() {
  if (process.env.WSL_INTEROP || process.env.WSL_DISTRO_NAME) return true;
  if (process.platform !== 'linux') return false;

  try {
    const version = fs.readFileSync('/proc/version', 'utf8');
    return /microsoft/i.test(version);
  } catch {
    return false;
  }
}

function runCargo(cargoArgs) {
  const result = spawnSync('cargo', cargoArgs, { stdio: 'inherit' });

  if (result.error) {
    process.stderr.write(`Failed to run cargo: ${result.error.message}\n`);
    process.exit(1);
  }

  process.exit(result.status ?? 1);
}

const argv = process.argv.slice(2);
if (argv.length === 0) usage();

const subcommand = argv.shift();
const targetArgs = isWsl() ? ['--target', 'x86_64-pc-windows-gnu'] : [];

switch (subcommand) {
  case 'dev': {
    // 约定：dev 的额外参数全部透传给应用参数（位于 `--` 之后）。
    //
    // 注意：存档目录用环境变量注入（硬切换：不再透传 `--save-dir`）。
    const env = { ...process.env, CRUFT_SAVE_DIR: './.dev/run' };
    const result = spawnSync('cargo', ['run', ...targetArgs, '--', ...argv], {
      stdio: 'inherit',
      env,
    });

    if (result.error) {
      process.stderr.write(`Failed to run cargo: ${result.error.message}\n`);
      process.exit(1);
    }

    process.exit(result.status ?? 1);
    break;
  }
  case 'test': {
    runCargo(['nextest', 'run', ...targetArgs, '--color=never', ...argv]);
    break;
  }
  case 'check': {
    runCargo(['check', ...targetArgs, ...argv]);
    break;
  }
  case 'clippy': {
    runCargo(['clippy', ...targetArgs, ...argv]);
    break;
  }
  case 'fmt': {
    // cargo fmt 不支持 --target
    runCargo(['fmt', ...argv]);
    break;
  }
  default: {
    usage();
  }
}
