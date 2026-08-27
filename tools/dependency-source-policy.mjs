import { parsePnpmLockYaml } from "./restricted-yaml.mjs";

const dependencyGroups = ["dependencies", "devDependencies", "optionalDependencies"];
const gitSchemePattern = /(?:^|[@(])(?:git(?:\+[a-z0-9+.-]+)?:|git:\/\/|ssh:\/\/|git@)/iu;
const gitDistributionHosts = new Set([
  "github.com",
  "gist.github.com",
  "codeload.github.com",
  "gitlab.com",
  "bitbucket.org",
]);
const gitHostReferencePattern = /(?:^|[@(])(?:https?:\/\/)?(?:www\.)?(?:github\.com|gist\.github\.com|codeload\.github\.com|gitlab\.com|bitbucket\.org)\//iu;
const sourceUrlPattern = /(?:https?|git|ssh):\/\/[^\s)]+/giu;
const gitFileSuffixPattern = /(?:^|[@(])https?:\/\/[^\s#]+\.git(?:[#)]|$)/iu;
const hostedShorthandPattern = /(?:^|[@(])(?:github|gitlab|bitbucket|gist):/iu;
const bareGithubShorthandPattern = /^[a-z0-9_.-]+\/[a-z0-9_.-]+(?:#[^\s]+)?$/iu;
const insecureHttpPattern = /(?:^|[@(])http:\/\//iu;
const maximumResolutionDepth = 16;

function fail(path, value, reason) {
  throw new Error(`${path}: ${reason}: ${JSON.stringify(value)}`);
}

function canonicalHostname(hostname) {
  return hostname
    .toLowerCase()
    .replace(/\.$/u, "")
    .replace(/^www\./u, "");
}

function hasForbiddenGitDistributionHost(reference) {
  for (const match of reference.matchAll(sourceUrlPattern)) {
    const candidate = match[0];
    try {
      if (gitDistributionHosts.has(canonicalHostname(new URL(candidate).hostname))) {
        return true;
      }
    } catch {
      // A malformed URL still falls through to the bounded spelling check below.
    }
  }
  return gitHostReferencePattern.test(reference);
}

function inspectReference(value, path, { allowBareShorthand = false } = {}) {
  if (typeof value !== "string") {
    fail(path, value, "dependency source reference must be a string");
  }
  const reference = value.trim();
  if (reference.length === 0) {
    fail(path, value, "dependency source reference must be nonempty");
  }
  if (
    insecureHttpPattern.test(reference) ||
    gitSchemePattern.test(reference) ||
    hasForbiddenGitDistributionHost(reference) ||
    gitFileSuffixPattern.test(reference) ||
    hostedShorthandPattern.test(reference) ||
    (allowBareShorthand && bareGithubShorthandPattern.test(reference))
  ) {
    fail(path, value, "insecure HTTP or Git dependency sources are forbidden");
  }
}

function requireExactFields(value, path, allowedFields, requiredFields = []) {
  const fields = Object.keys(value);
  const unexpected = fields.filter((field) => !allowedFields.includes(field));
  if (unexpected.length > 0) {
    fail(path, value, `unsupported fields: ${unexpected.join(", ")}`);
  }
  const missing = requiredFields.filter((field) => !Object.hasOwn(value, field));
  if (missing.length > 0) {
    fail(path, value, `missing required fields: ${missing.join(", ")}`);
  }
}

function requireNonemptyString(value, path, description) {
  if (typeof value !== "string" || value.trim().length === 0) {
    fail(path, value, `${description} must be a nonempty string`);
  }
  return value;
}

function inspectBinaryBin(value, path) {
  if (typeof value === "string") {
    requireNonemptyString(value, path, "binary executable path");
    return;
  }
  const bins = requireRecord(value, path);
  if (Object.keys(bins).length === 0) {
    fail(path, value, "binary executable mapping must be nonempty");
  }
  for (const [name, executablePath] of Object.entries(bins)) {
    requireNonemptyString(name, `${path} key`, "binary executable name");
    requireNonemptyString(executablePath, `${path}.${name}`, "binary executable path");
  }
}

function inspectVariationTarget(target, path) {
  const record = requireRecord(target, path);
  requireExactFields(record, path, ["os", "cpu", "libc"], ["os", "cpu"]);
  requireNonemptyString(record.os, `${path}.os`, "variation target os");
  requireNonemptyString(record.cpu, `${path}.cpu`, "variation target cpu");
  if (Object.hasOwn(record, "libc")) {
    requireNonemptyString(record.libc, `${path}.libc`, "variation target libc");
  }
}

function inspectResolution(resolution, path, depth = 0) {
  if (depth > maximumResolutionDepth) {
    fail(path, resolution, `resolution nesting exceeds ${maximumResolutionDepth}`);
  }
  if (typeof resolution === "string") {
    inspectReference(resolution, path, { allowBareShorthand: true });
    return;
  }
  const record = requireRecord(resolution, path);
  if (!Object.hasOwn(record, "type")) {
    requireExactFields(record, path, ["integrity", "tarball"]);
    if (!Object.hasOwn(record, "integrity") && !Object.hasOwn(record, "tarball")) {
      fail(path, resolution, "registry or tarball resolution must identify its source");
    }
    if (Object.hasOwn(record, "integrity")) {
      requireNonemptyString(record.integrity, `${path}.integrity`, "resolution integrity");
    }
    if (Object.hasOwn(record, "tarball")) {
      requireNonemptyString(record.tarball, `${path}.tarball`, "tarball source");
      inspectReference(record.tarball, `${path}.tarball`);
    }
    return;
  }

  const type = requireNonemptyString(record.type, `${path}.type`, "resolution type");
  if (type === "git") {
    fail(`${path}.type`, type, "Git resolution type is forbidden");
  }
  if (type === "directory") {
    requireExactFields(record, path, ["type", "directory"], ["type", "directory"]);
    requireNonemptyString(record.directory, `${path}.directory`, "directory resolution path");
    inspectReference(record.directory, `${path}.directory`);
    return;
  }
  if (type === "binary") {
    requireExactFields(
      record,
      path,
      ["type", "url", "integrity", "bin", "archive", "prefix"],
      ["type", "url", "integrity", "bin", "archive"],
    );
    requireNonemptyString(record.url, `${path}.url`, "binary source URL");
    inspectReference(record.url, `${path}.url`);
    requireNonemptyString(record.integrity, `${path}.integrity`, "binary integrity");
    inspectBinaryBin(record.bin, `${path}.bin`);
    if (record.archive !== "zip" && record.archive !== "tarball") {
      fail(`${path}.archive`, record.archive, "binary archive must be zip or tarball");
    }
    if (Object.hasOwn(record, "prefix")) {
      requireNonemptyString(record.prefix, `${path}.prefix`, "binary archive prefix");
    }
    return;
  }
  if (type === "variations") {
    requireExactFields(record, path, ["type", "variants"], ["type", "variants"]);
    if (!Array.isArray(record.variants) || record.variants.length === 0) {
      fail(`${path}.variants`, record.variants, "variation resolutions must be a nonempty array");
    }
    for (const [variantIndex, variant] of record.variants.entries()) {
      const variantPath = `${path}.variants[${variantIndex}]`;
      const variantRecord = requireRecord(variant, variantPath);
      requireExactFields(
        variantRecord,
        variantPath,
        ["targets", "resolution"],
        ["targets", "resolution"],
      );
      if (!Array.isArray(variantRecord.targets) || variantRecord.targets.length === 0) {
        fail(
          `${variantPath}.targets`,
          variantRecord.targets,
          "variation targets must be a nonempty array",
        );
      }
      for (const [targetIndex, target] of variantRecord.targets.entries()) {
        inspectVariationTarget(target, `${variantPath}.targets[${targetIndex}]`);
      }
      inspectResolution(variantRecord.resolution, `${variantPath}.resolution`, depth + 1);
    }
    return;
  }
  fail(`${path}.type`, type, "unsupported resolution type");
}

function inspectDependencyEntry(entry, path) {
  if (typeof entry === "string") {
    inspectReference(entry, path, { allowBareShorthand: true });
    return;
  }
  if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
    fail(path, entry, "dependency entry must be a string or object");
  }
  const fields = Object.keys(entry);
  if (
    fields.length === 0 ||
    fields.some((field) => field !== "specifier" && field !== "version")
  ) {
    fail(path, entry, "dependency object must contain only specifier and/or version");
  }
  for (const field of fields) {
    inspectReference(entry[field], `${path}.${field}`, { allowBareShorthand: true });
  }
}

function inspectCatalogEntry(entry, path) {
  if (typeof entry === "string") {
    if (entry.trim().length === 0) {
      fail(path, entry, "catalog source reference must be nonempty");
    }
    inspectReference(entry, path, { allowBareShorthand: true });
    return;
  }
  if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
    fail(path, entry, "catalog entry must be a string or specifier/version object");
  }
  const fields = Object.keys(entry);
  if (
    fields.length === 0 ||
    fields.some((field) => field !== "specifier" && field !== "version")
  ) {
    fail(path, entry, "catalog entry must contain only specifier and/or version");
  }
  for (const field of fields) {
    if (typeof entry[field] === "string" && entry[field].trim().length === 0) {
      fail(`${path}.${field}`, entry[field], "catalog source reference must be nonempty");
    }
    inspectReference(entry[field], `${path}.${field}`, { allowBareShorthand: true });
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
  const lock = requireRecord(parsePnpmLockYaml(lockText, label), label);

  if (lock.catalogs !== undefined) {
    const catalogs = requireRecord(lock.catalogs, `${label}.catalogs`);
    for (const [catalogName, catalogValue] of Object.entries(catalogs)) {
      const catalog = requireRecord(catalogValue, `${label}.catalogs.${catalogName}`);
      for (const [dependencyName, entry] of Object.entries(catalog)) {
        inspectCatalogEntry(entry, `${label}.catalogs.${catalogName}.${dependencyName}`);
      }
    }
  }

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
