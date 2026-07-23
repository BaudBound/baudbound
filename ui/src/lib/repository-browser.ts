export type RepositoryScriptState =
  | "incompatible"
  | "installed"
  | "installed_elsewhere"
  | "not_installed"
  | "unavailable"
  | "update_available";

export function repositoryDisplayName(source: {
  name: string;
  official: boolean;
}) {
  const name = source.name.trim();
  if (name) return name;
  return source.official
    ? "BaudBound Official Repository"
    : "Repository awaiting first refresh";
}

export function repositoryUrlForDisplay(value: string) {
  try {
    const url = new URL(value);
    if (url.search) url.search = "?redacted";
    return url.toString();
  } catch {
    return "Invalid repository URL";
  }
}

export function repositoryScriptState({
  compatible,
  informationMismatch,
  installed,
  installedFromThisRepository,
  updateAvailable,
}: {
  compatible: boolean;
  informationMismatch: boolean;
  installed: boolean;
  installedFromThisRepository: boolean;
  updateAvailable: boolean;
}): RepositoryScriptState {
  if (informationMismatch) return "unavailable";
  if (!compatible) return "incompatible";
  if (!installed) return "not_installed";
  if (!installedFromThisRepository) return "installed_elsewhere";
  if (updateAvailable) return "update_available";
  return "installed";
}

export function compareSemanticVersions(left: string, right: string) {
  const parsedLeft = parseSemanticVersion(left);
  const parsedRight = parseSemanticVersion(right);
  if (!parsedLeft || !parsedRight) return left.localeCompare(right);
  const leftNumbers = parsedLeft.slice(0, 3) as number[];
  const rightNumbers = parsedRight.slice(0, 3) as number[];
  for (let index = 0; index < leftNumbers.length; index += 1) {
    const difference = leftNumbers[index] - rightNumbers[index];
    if (difference !== 0) return difference;
  }
  if (parsedLeft[3] === parsedRight[3]) return 0;
  if (parsedLeft[3] === "") return 1;
  if (parsedRight[3] === "") return -1;
  return parsedLeft[3].localeCompare(parsedRight[3], undefined, {
    numeric: true,
    sensitivity: "base",
  });
}

export function meetsMinimumRunnerVersion(
  runnerVersion: string,
  minimumRunnerVersion: string,
) {
  return compareSemanticVersions(runnerVersion, minimumRunnerVersion) >= 0;
}

function parseSemanticVersion(
  value: string,
): [number, number, number, string] | null {
  const match =
    /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(
      value,
    );
  if (!match) return null;
  return [
    Number(match[1]),
    Number(match[2]),
    Number(match[3]),
    match[4] ?? "",
  ];
}
