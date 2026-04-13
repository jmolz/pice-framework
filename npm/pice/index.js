"use strict";

const path = require("path");
const { execFileSync } = require("child_process");
const fs = require("fs");

/**
 * Platform/arch to npm package name mapping.
 * Linux packages contain glibc-linked binaries. musl-based distros
 * (Alpine, Void, etc.) are detected and rejected with a clear message.
 */
const PLATFORM_MAP = {
  "darwin-arm64": { pkg: "@pice/pice-darwin-arm64", bin: "pice", daemonBin: "pice-daemon" },
  "darwin-x64": { pkg: "@pice/pice-darwin-x64", bin: "pice", daemonBin: "pice-daemon" },
  "linux-arm64": { pkg: "@pice/pice-linux-arm64", bin: "pice", daemonBin: "pice-daemon" },
  "linux-x64": { pkg: "@pice/pice-linux-x64", bin: "pice", daemonBin: "pice-daemon" },
  "win32-x64": { pkg: "@pice/pice-win32-x64", bin: "pice.exe", daemonBin: "pice-daemon.exe" },
};

/**
 * Detect whether the current Linux system uses musl libc.
 * Uses execFileSync (no shell) with a hardcoded binary path for safety.
 */
function isMusl() {
  if (process.platform !== "linux") return false;
  try {
    // execFileSync avoids shell injection — hardcoded binary, no user input
    const output = execFileSync("ldd", ["--version"], {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    return output.toLowerCase().includes("musl");
  } catch (e) {
    // ldd writes to stderr on some systems; check combined output
    if (e.stderr && e.stderr.toLowerCase().includes("musl")) return true;
    // Fallback: check for Alpine marker file
    try {
      const release = fs.readFileSync("/etc/os-release", "utf8");
      return release.toLowerCase().includes("alpine");
    } catch {
      return false;
    }
  }
}

/**
 * Returns the absolute path to the PICE binary for the current platform.
 *
 * @returns {string} Absolute path to the pice binary
 * @throws {Error} If the current platform/arch combination is unsupported
 *   or if the platform-specific package is not installed
 */
function getBinaryPath() {
  if (isMusl()) {
    throw new Error(
      `PICE CLI does not currently provide musl/Alpine Linux binaries. ` +
        `The npm packages contain glibc-linked binaries that will not work ` +
        `on musl-based systems. Install from source instead: ` +
        `cargo install pice-cli`
    );
  }

  const key = `${process.platform}-${process.arch}`;
  const entry = PLATFORM_MAP[key];

  if (!entry) {
    const supported = Object.keys(PLATFORM_MAP)
      .map((k) => k.replace("-", "/"))
      .join(", ");
    throw new Error(
      `Unsupported platform: ${process.platform}/${process.arch}. ` +
        `PICE CLI supports: ${supported}. ` +
        `If you believe this is a bug, please open an issue at ` +
        `https://github.com/jacobmolz/pice/issues`
    );
  }

  let pkgDir;
  try {
    const pkgJsonPath = require.resolve(`${entry.pkg}/package.json`);
    pkgDir = path.dirname(pkgJsonPath);
  } catch {
    throw new Error(
      `The platform-specific package ${entry.pkg} is not installed. ` +
        `This usually means your package manager did not install the ` +
        `optional dependency for your platform. ` +
        `Try reinstalling with: npm install pice`
    );
  }

  return path.join(pkgDir, entry.bin);
}

/**
 * Returns the absolute path to the pice-daemon binary for the current platform.
 *
 * The daemon binary ships alongside the CLI in the same platform package.
 * The CLI's auto-start logic uses this to locate `pice-daemon` without
 * requiring it to be on the user's `$PATH`.
 *
 * @returns {string} Absolute path to the pice-daemon binary
 * @throws {Error} If the current platform/arch combination is unsupported
 *   or if the platform-specific package is not installed
 */
function getDaemonBinaryPath() {
  if (isMusl()) {
    throw new Error(
      `PICE daemon does not currently provide musl/Alpine Linux binaries. ` +
        `Install from source instead: cargo install pice-daemon`
    );
  }

  const key = `${process.platform}-${process.arch}`;
  const entry = PLATFORM_MAP[key];

  if (!entry) {
    throw new Error(
      `Unsupported platform: ${process.platform}/${process.arch}.`
    );
  }

  let pkgDir;
  try {
    const pkgJsonPath = require.resolve(`${entry.pkg}/package.json`);
    pkgDir = path.dirname(pkgJsonPath);
  } catch {
    throw new Error(
      `The platform-specific package ${entry.pkg} is not installed.`
    );
  }

  return path.join(pkgDir, entry.daemonBin);
}

module.exports = { getBinaryPath, getDaemonBinaryPath };
