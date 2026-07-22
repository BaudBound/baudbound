import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const PACKAGE_NAME = "baud-bound";
const REQUIRED_FILES = [
  "/usr/bin/baudbound",
  "/usr/lib/BaudBound/CONTENT-LICENSE.md",
  "/usr/lib/BaudBound/LICENSE.md",
  "/usr/lib/BaudBound/TRADEMARKS.md",
  "/usr/share/applications/BaudBound.desktop",
  "/usr/share/icons/hicolor/32x32/apps/baudbound.png",
  "/usr/share/icons/hicolor/128x128/apps/baudbound.png",
  "/usr/share/icons/hicolor/256x256@2/apps/baudbound.png",
];
const DEB_DEPENDENCIES = [
  "libasound2",
  "libayatana-appindicator3-1",
  "libgtk-3-0",
  "libudev1",
  "libwebkit2gtk-4.1-0",
];
const RPM_DEPENDENCIES = [
  "libasound.so.2()(64bit)",
  "libayatana-appindicator3.so.1()(64bit)",
  "libgtk-3.so.0()(64bit)",
  "libudev.so.1()(64bit)",
  "libwebkit2gtk-4.1.so.0()(64bit)",
];

export class LinuxPackageContractError extends Error {}

export function verifyLinuxPackages({ directory, tag }) {
  const version = releaseVersion(tag);
  const packageDirectory = resolve(directory);
  const debPath = exactlyOnePackage(packageDirectory, ".deb");
  const rpmPath = exactlyOnePackage(packageDirectory, ".rpm");
  const extractionRoot = mkdtempSync(join(tmpdir(), "baudbound-package-check-"));

  try {
    verifyDebPackage(debPath, version, join(extractionRoot, "deb"));
    verifyRpmPackage(rpmPath, version);
  } finally {
    rmSync(extractionRoot, { force: true, recursive: true });
  }

  return {
    deb: basename(debPath),
    rpm: basename(rpmPath),
    version,
  };
}

function verifyDebPackage(packagePath, version, extractionDirectory) {
  assert(
    basename(packagePath) === `BaudBound_${version}_amd64.deb`,
    `unexpected Debian package filename ${basename(packagePath)}`,
  );
  assertField("Debian package name", debField(packagePath, "Package"), PACKAGE_NAME);
  assertField("Debian version", debField(packagePath, "Version"), version);
  assertField("Debian architecture", debField(packagePath, "Architecture"), "amd64");
  assertField("Debian maintainer", debField(packagePath, "Maintainer"), "NATroutter");
  assertField("Debian section", debField(packagePath, "Section"), "utils");
  assertField("Debian priority", debField(packagePath, "Priority"), "optional");
  assertField("Debian homepage", debField(packagePath, "Homepage"), "https://baudbound.app");
  assertDependencies(
    "Debian",
    debField(packagePath, "Depends").split(",").map((value) => value.trim()),
    DEB_DEPENDENCIES,
  );

  run("dpkg-deb", ["--extract", packagePath, extractionDirectory]);
  const controlDirectory = join(extractionDirectory, "DEBIAN");
  run("dpkg-deb", ["--control", packagePath, controlDirectory]);
  const unexpectedScripts = readdirSync(controlDirectory).filter((name) =>
    ["preinst", "postinst", "prerm", "postrm"].includes(name),
  );
  assert(
    unexpectedScripts.length === 0,
    `Debian package contains unexpected maintainer scripts: ${unexpectedScripts.join(", ")}`,
  );
  verifyInstalledTree(extractionDirectory, "Debian");
}

function verifyRpmPackage(packagePath, version) {
  assert(
    basename(packagePath) === `BaudBound-${version}-1.x86_64.rpm`,
    `unexpected RPM package filename ${basename(packagePath)}`,
  );
  assertField("RPM package name", rpmField(packagePath, "%{NAME}"), PACKAGE_NAME);
  assertField("RPM epoch", rpmField(packagePath, "%{EPOCHNUM}"), "0");
  assertField("RPM version", rpmField(packagePath, "%{VERSION}"), version);
  assertField("RPM release", rpmField(packagePath, "%{RELEASE}"), "1");
  assertField("RPM architecture", rpmField(packagePath, "%{ARCH}"), "x86_64");
  assertField(
    "RPM license",
    rpmField(packagePath, "%{LICENSE}"),
    "PolyForm-Noncommercial-1.0.0",
  );
  assertField("RPM homepage", rpmField(packagePath, "%{URL}"), "https://baudbound.app");
  assertDependencies(
    "RPM",
    run("rpm", ["-qp", "--requires", packagePath]).split(/\r?\n/).filter(Boolean),
    RPM_DEPENDENCIES,
  );
  assert(
    run("rpm", ["-qp", "--scripts", packagePath]).trim() === "",
    "RPM package contains unexpected install or removal scripts",
  );

  const installedFiles = run("rpm", ["-qpl", packagePath])
    .split(/\r?\n/)
    .filter(Boolean);
  for (const installedPath of REQUIRED_FILES) {
    assert(installedFiles.includes(installedPath), `RPM package is missing ${installedPath}`);
  }
}

function verifyInstalledTree(extractionDirectory, format) {
  for (const installedPath of REQUIRED_FILES) {
    const filePath = join(extractionDirectory, installedPath.slice(1));
    assert(statSync(filePath).isFile(), `${format} package is missing ${installedPath}`);
    assert(statSync(filePath).size > 0, `${format} package contains an empty ${installedPath}`);
  }

  const executable = join(extractionDirectory, "usr/bin/baudbound");
  assert(
    (statSync(executable).mode & 0o111) !== 0,
    `${format} package executable is not marked executable`,
  );

  const desktopEntry = readFileSync(
    join(extractionDirectory, "usr/share/applications/BaudBound.desktop"),
    "utf8",
  );
  for (const line of [
    "Type=Application",
    "Name=BaudBound",
    "Exec=baudbound",
    "Icon=baudbound",
    "Terminal=false",
    "Categories=Utility;",
  ]) {
    assert(
      desktopEntry.split(/\r?\n/).includes(line),
      `${format} desktop entry is missing ${line}`,
    );
  }
}

function releaseVersion(tag) {
  assert(typeof tag === "string", "release tag is required");
  const match = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/.exec(tag);
  assert(match, `invalid release tag ${tag}`);
  return match[1];
}

function exactlyOnePackage(directory, suffix) {
  const packages = packageFiles(directory, suffix);
  assert(packages.length === 1, `expected exactly one ${suffix} package, found ${packages.length}`);
  return packages[0];
}

function packageFiles(directory, suffix) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      return packageFiles(path, suffix);
    }
    return entry.isFile() && entry.name.endsWith(suffix) ? [path] : [];
  });
}

function debField(packagePath, field) {
  return run("dpkg-deb", ["--field", packagePath, field]).trim();
}

function rpmField(packagePath, field) {
  return run("rpm", ["-qp", "--queryformat", field, packagePath]).trim();
}

function assertField(label, actual, expected) {
  assert(actual === expected, `${label} must be ${expected}, found ${actual || "an empty value"}`);
}

function assertDependencies(format, actual, required) {
  const duplicates = actual.filter((dependency, index) => actual.indexOf(dependency) !== index);
  assert(
    duplicates.length === 0,
    `${format} package contains duplicate dependencies: ${[...new Set(duplicates)].join(", ")}`,
  );
  const missing = required.filter((dependency) => !actual.includes(dependency));
  assert(
    missing.length === 0,
    `${format} package is missing dependencies: ${missing.join(", ")}`,
  );
}

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    ...options,
  });
  assertCommand(command, result);
  return result.stdout;
}

function assertCommand(command, result) {
  if (result.error) {
    throw new LinuxPackageContractError(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const details = Buffer.isBuffer(result.stderr)
      ? result.stderr.toString("utf8").trim()
      : result.stderr?.trim();
    throw new LinuxPackageContractError(
      `${command} failed with exit code ${result.status}${details ? `: ${details}` : ""}`,
    );
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new LinuxPackageContractError(message);
  }
}
