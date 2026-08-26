import YAML from "yaml";

const dependencyGroups = ["dependencies", "devDependencies", "optionalDependencies"];
const gitSchemePattern = /(?:^|[@(])(?:git(?:\+[a-z0-9+.-]+)?:|git:\/\/|ssh:\/\/|git@)/iu;
const gitHostPattern = /(?:^|[@(])(?:https?:\/\/)?(?:www\.)?(?:github\.com|gitlab\.com|bitbucket\.org)\//iu;
const gitFileSuffixPattern = /(?:^|[@(])https?:\/\/[^\s#]+\.git(?:[#)]|$)/iu;
const hostedShorthandPattern = /(?:^|[@(])(?:github|gitlab|bitbucket):/iu;
const bareGithubShorthandPattern = /^[a-z0-9_.-]+\/[a-z0-9_.-]+(?:#[^\s]+)?$/iu;
const insecureHttpPattern = /(?:^|[@(])http:\/\//iu;

function fail(path, value, reason) {
  throw new Error(`${path}: ${reason}: ${JSON.stringify(value)}`);
}

function inspectReference(value, path, { allowBareShorthand = false } = {}) {
  if (typeof value !== "string") {
    fail(path, value, "dependency source reference must be a string");
  }
  const reference = value.trim();
  if (
    insecureHttpPattern.test(reference) ||
    gitSchemePattern.test(reference) ||
    gitHostPattern.test(reference) ||
    gitFileSuffixPattern.test(reference) ||
    hostedShorthandPattern.test(reference) ||
    (allowBareShorthand && bareGithubShorthandPattern.test(reference))
  ) {
    fail(path, value, "insecure HTTP or Git dependency sources are forbidden");
  }
}

function inspectResolution(resolution, path) {
  if (typeof resolution === "string") {
    inspectReference(resolution, path, { allowBareShorthand: true });
    return;
  }
  if (typeof resolution !== "object" || resolution === null || Array.isArray(resolution)) {
    fail(path, resolution, "resolution must be a source object or string");
  }
  if (Object.hasOwn(resolution, "repo")) {
    fail(`${path}.repo`, resolution.repo, "repository resolutions are forbidden");
  }
  if (typeof resolution.type === "string" && resolution.type.toLowerCase() === "git") {
    fail(`${path}.type`, resolution.type, "Git resolution type is forbidden");
  }
  if (Object.hasOwn(resolution, "tarball")) {
    if (typeof resolution.tarball !== "string") {
      fail(`${path}.tarball`, resolution.tarball, "tarball source must be a string");
    }
    const tarball = resolution.tarball.trim();
    if (/^http:\/\//iu.test(tarball)) {
      fail(`${path}.tarball`, resolution.tarball, "insecure HTTP tarballs are forbidden");
    }
    inspectReference(tarball, `${path}.tarball`);
  }
}

function inspectDependencyEntry(entry, path) {
  if (typeof entry === "string") {
    inspectReference(entry, path, { allowBareShorthand: true });
    return;
  }
  if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
    fail(path, entry, "dependency entry must be a string or object");
  }
  for (const field of ["specifier", "version"]) {
    if (Object.hasOwn(entry, field)) {
      inspectReference(entry[field], `${path}.${field}`, { allowBareShorthand: true });
    }
  }
}

function requireRecord(value, path) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail(path, value, "must be a mapping");
  }
  return value;
}

/** Parse a pnpm lock structurally and reject disallowed source encodings. */
export function assertPnpmLockSourcePolicy(lockText, label = "pnpm-lock.yaml") {
  const document = YAML.parseDocument(lockText, {
    prettyErrors: true,
    strict: true,
    uniqueKeys: true,
  });
  if (document.errors.length > 0) {
    throw new Error(`${label}: invalid YAML: ${document.errors.map((error) => error.message).join("; ")}`);
  }
  if (document.warnings.length > 0) {
    throw new Error(`${label}: ambiguous YAML warning: ${document.warnings.map((warning) => warning.message).join("; ")}`);
  }
  const lock = requireRecord(document.toJS({ maxAliasCount: 0 }), label);

  if (lock.importers !== undefined) {
    const importers = requireRecord(lock.importers, `${label}.importers`);
    for (const [importerName, importerValue] of Object.entries(importers)) {
      const importer = requireRecord(importerValue, `${label}.importers.${importerName}`);
      for (const groupName of dependencyGroups) {
        if (importer[groupName] === undefined) {
          continue;
        }
        const group = requireRecord(
          importer[groupName],
          `${label}.importers.${importerName}.${groupName}`,
        );
        for (const [dependencyName, entry] of Object.entries(group)) {
          inspectDependencyEntry(
            entry,
            `${label}.importers.${importerName}.${groupName}.${dependencyName}`,
          );
        }
      }
    }
  }

  for (const sectionName of ["packages", "snapshots"]) {
    if (lock[sectionName] === undefined) {
      continue;
    }
    const section = requireRecord(lock[sectionName], `${label}.${sectionName}`);
    for (const [packageKey, packageValue] of Object.entries(section)) {
      inspectReference(packageKey, `${label}.${sectionName} key`, { allowBareShorthand: false });
      const packageRecord = requireRecord(
        packageValue,
        `${label}.${sectionName}.${packageKey}`,
      );
      if (Object.hasOwn(packageRecord, "resolution")) {
        inspectResolution(
          packageRecord.resolution,
          `${label}.${sectionName}.${packageKey}.resolution`,
        );
      }
      for (const groupName of dependencyGroups) {
        if (packageRecord[groupName] === undefined) {
          continue;
        }
        const group = requireRecord(
          packageRecord[groupName],
          `${label}.${sectionName}.${packageKey}.${groupName}`,
        );
        for (const [dependencyName, reference] of Object.entries(group)) {
          inspectDependencyEntry(
            reference,
            `${label}.${sectionName}.${packageKey}.${groupName}.${dependencyName}`,
          );
        }
      }
    }
  }
}
