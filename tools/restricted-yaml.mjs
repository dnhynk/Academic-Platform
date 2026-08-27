const forbiddenPlainScalarPrefixes = ["&", "*", "!"];

function fail(label, line, message) {
  const location = line === undefined ? label : `${label}:${line}`;
  throw new Error(`${location}: unsupported or invalid lockfile YAML: ${message}`);
}

function assertUnicodeScalars(value, label, line) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        fail(label, line, "string contains an unpaired high surrogate");
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      fail(label, line, "string contains an unpaired low surrogate");
    }
  }
  return value;
}

function decodeDoubleQuoted(value, label, line) {
  let result = "";
  for (let index = 1; index < value.length - 1; index += 1) {
    const character = value[index] ?? "";
    if (character !== "\\") {
      if (character === '"') {
        fail(label, line, "unescaped quote inside a double-quoted scalar");
      }
      result += character;
      continue;
    }
    index += 1;
    const escape = value[index] ?? "";
    const simple = new Map([
      ["0", "\0"],
      ["a", "\u0007"],
      ["b", "\b"],
      ["t", "\t"],
      ["n", "\n"],
      ["v", "\u000b"],
      ["f", "\f"],
      ["r", "\r"],
      ["e", "\u001b"],
      [" ", " "],
      ['"', '"'],
      ["/", "/"],
      ["\\", "\\"],
      ["N", "\u0085"],
      ["_", "\u00a0"],
      ["L", "\u2028"],
      ["P", "\u2029"],
    ]);
    if (simple.has(escape)) {
      result += simple.get(escape);
      continue;
    }
    const digits = escape === "x" ? 2 : escape === "u" ? 4 : escape === "U" ? 8 : 0;
    if (digits === 0) {
      fail(label, line, `unknown double-quoted escape \\${escape}`);
    }
    const hex = value.slice(index + 1, index + 1 + digits);
    if (hex.length !== digits || !/^[0-9a-f]+$/iu.test(hex)) {
      fail(label, line, `invalid \\${escape} Unicode escape`);
    }
    const codePoint = Number.parseInt(hex, 16);
    if (codePoint > 0x10ffff || (codePoint >= 0xd800 && codePoint <= 0xdfff)) {
      fail(label, line, "escape does not encode a Unicode scalar value");
    }
    result += String.fromCodePoint(codePoint);
    index += digits;
  }
  return assertUnicodeScalars(result, label, line);
}

function decodeQuotedScalar(value, label, line) {
  if (value.startsWith("'")) {
    if (!value.endsWith("'") || value.length < 2) {
      fail(label, line, "unterminated single-quoted scalar");
    }
    let result = "";
    for (let index = 1; index < value.length - 1; index += 1) {
      if (value[index] === "'") {
        if (value[index + 1] !== "'") {
          fail(label, line, "single quote inside a quoted scalar must be doubled");
        }
        result += "'";
        index += 1;
      } else {
        result += value[index];
      }
    }
    return assertUnicodeScalars(result, label, line);
  }
  if (value.startsWith('"')) {
    if (!value.endsWith('"') || value.length < 2) {
      fail(label, line, "unterminated double-quoted scalar");
    }
    return decodeDoubleQuoted(value, label, line);
  }
  fail(label, line, "internal quoted-scalar parser error");
}

function stripComment(value, label, line) {
  let quote = null;
  let escaped = false;
  for (let index = 0; index < value.length; index += 1) {
    const character = value[index] ?? "";
    if (quote === '"') {
      if (escaped) {
        escaped = false;
      } else if (character === "\\") {
        escaped = true;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (quote === "'") {
      if (character === quote && value[index + 1] === quote) {
        index += 1;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "#" && (index === 0 || /\s/u.test(value[index - 1] ?? ""))) {
      return value.slice(0, index).trimEnd();
    }
  }
  if (quote !== null) {
    fail(label, line, "multiline quoted scalars are outside the lockfile profile");
  }
  return value.trimEnd();
}

function findMappingColon(value, { flow = false } = {}) {
  let quote = null;
  let escaped = false;
  let depth = 0;
  for (let index = 0; index < value.length; index += 1) {
    const character = value[index] ?? "";
    if (quote === '"') {
      if (escaped) {
        escaped = false;
      } else if (character === "\\") {
        escaped = true;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (quote === "'") {
      if (character === quote && value[index + 1] === quote) {
        index += 1;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "[" || character === "{") {
      depth += 1;
    } else if (character === "]" || character === "}") {
      depth -= 1;
    } else if (
      character === ":" &&
      depth === 0 &&
      (flow || index === value.length - 1 || /\s/u.test(value[index + 1] ?? ""))
    ) {
      return index;
    }
  }
  return -1;
}

function parseKey(value, label, line) {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    fail(label, line, "mapping key must be nonempty");
  }
  if (forbiddenPlainScalarPrefixes.some((prefix) => trimmed.startsWith(prefix))) {
    fail(label, line, "anchors, aliases, and tags are forbidden on mapping keys");
  }
  const key = trimmed.startsWith("'") || trimmed.startsWith('"')
    ? decodeQuotedScalar(trimmed, label, line)
    : assertUnicodeScalars(trimmed, label, line);
  if (key === "<<") {
    fail(label, line, "merge keys are forbidden");
  }
  return key;
}

const yamlDatePattern = /^([0-9]{4})-([0-9]{2})-([0-9]{2})$/u;
const yamlTimestampPattern = /^([0-9]{4})-([0-9]{1,2})-([0-9]{1,2})(?:[Tt]|[ \t]+)([0-9]{1,2}):([0-9]{2}):([0-9]{2})(?:\.([0-9]*))?(?:[ \t]*(Z|([-+])([0-9]{1,2})(?::([0-9]{2}))?))?$/u;

function decodePlainTimestamp(value) {
  const match = value.match(yamlDatePattern) ?? value.match(yamlTimestampPattern);
  if (match === null) return undefined;
  const year = Number(match[1]);
  const month = Number(match[2]) - 1;
  const day = Number(match[3]);
  if (match[4] === undefined) {
    return new Date(Date.UTC(year, month, day));
  }
  const hour = Number(match[4]);
  const minute = Number(match[5]);
  const second = Number(match[6]);
  const fractionText = (match[7] ?? "").slice(0, 3).padEnd(3, "0");
  const fraction = Number(fractionText);
  const date = new Date(Date.UTC(year, month, day, hour, minute, second, fraction));
  if (match[9] !== undefined) {
    const timezoneMinutes = Number(match[10]) * 60 + Number(match[11] ?? 0);
    const signedDelta = (match[9] === "-" ? -1 : 1) * timezoneMinutes * 60_000;
    date.setTime(date.getTime() - signedDelta);
  }
  return date;
}

function decodePlainInteger(value) {
  const sign = value.startsWith("-") ? -1 : 1;
  const unsigned = /^[+-]/u.test(value) ? value.slice(1) : value;
  const lowercase = unsigned.toLowerCase();
  if (lowercase.startsWith("0b")) return sign * Number.parseInt(unsigned.slice(2), 2);
  if (lowercase.startsWith("0o")) return sign * Number.parseInt(unsigned.slice(2), 8);
  if (lowercase.startsWith("0x")) return sign * Number.parseInt(unsigned.slice(2), 16);
  return sign * Number.parseInt(unsigned, 10);
}

function decodePlainScalar(value, label, line) {
  const scalar = assertUnicodeScalars(value, label, line);
  if (scalar === "~" || /^null$/iu.test(scalar)) {
    return null;
  }
  if (/^true$/iu.test(scalar)) {
    return true;
  }
  if (/^false$/iu.test(scalar)) {
    return false;
  }
  const normalized = scalar.replaceAll("_", "");
  if (
    /^[-+]?[0-9]+$/u.test(normalized) ||
    /^[-+]?0b[01]+$/iu.test(normalized) ||
    /^[-+]?0o[0-7]+$/iu.test(normalized) ||
    /^[-+]?0x[0-9a-f]+$/iu.test(normalized)
  ) {
    return decodePlainInteger(normalized);
  }
  if (/^[-+]?\.(?:inf|nan)$/iu.test(normalized)) {
    if (/nan$/iu.test(normalized)) return Number.NaN;
    return normalized.startsWith("-") ? Number.NEGATIVE_INFINITY : Number.POSITIVE_INFINITY;
  }
  if (
    /^[-+]?(?:(?:[0-9]+\.[0-9]*|\.[0-9]+)(?:e[-+]?[0-9]+)?|[0-9]+e[-+]?[0-9]+)$/iu.test(
      normalized,
    )
  ) {
    return Number(normalized);
  }
  const timestamp = decodePlainTimestamp(scalar);
  if (timestamp !== undefined) return timestamp;
  return scalar;
}

class FlowParser {
  constructor(input, label, line) {
    this.input = input;
    this.label = label;
    this.line = line;
    this.index = 0;
  }

  error(message) {
    fail(this.label, this.line, message);
  }

  skipSpace() {
    while (/\s/u.test(this.input[this.index] ?? "")) {
      this.index += 1;
    }
  }

  parseQuoted() {
    const start = this.index;
    const quote = this.input[this.index] ?? "";
    this.index += 1;
    while (this.index < this.input.length) {
      const character = this.input[this.index] ?? "";
      if (quote === "'" && character === "'" && this.input[this.index + 1] === "'") {
        this.index += 2;
        continue;
      }
      if (quote === '"' && character === "\\") {
        this.index += 2;
        continue;
      }
      this.index += 1;
      if (character === quote) {
        return decodeQuotedScalar(this.input.slice(start, this.index), this.label, this.line);
      }
    }
    this.error("unterminated quoted flow scalar");
  }

  parsePlain(stoppers) {
    const start = this.index;
    while (this.index < this.input.length && !stoppers.has(this.input[this.index] ?? "")) {
      this.index += 1;
    }
    const value = this.input.slice(start, this.index).trim();
    if (value.length === 0) {
      this.error("flow scalar must be nonempty");
    }
    if (forbiddenPlainScalarPrefixes.some((prefix) => value.startsWith(prefix))) {
      this.error("anchors, aliases, and tags are forbidden");
    }
    if (value === "|" || value === ">") {
      this.error("block scalars are outside the lockfile profile");
    }
    return decodePlainScalar(value, this.label, this.line);
  }

  parseValue() {
    this.skipSpace();
    const character = this.input[this.index] ?? "";
    if (character === "{") {
      return this.parseMapping();
    }
    if (character === "[") {
      return this.parseSequence();
    }
    if (character === '"' || character === "'") {
      return this.parseQuoted();
    }
    return this.parsePlain(new Set([",", "}", "]"]));
  }

  parseFlowKey() {
    this.skipSpace();
    if (this.input[this.index] === '"' || this.input[this.index] === "'") {
      return this.parseQuoted();
    }
    const start = this.index;
    while (this.index < this.input.length && this.input[this.index] !== ":") {
      if (this.input[this.index] === "," || this.input[this.index] === "}") {
        this.error("flow mapping entry is missing ':'");
      }
      this.index += 1;
    }
    return parseKey(this.input.slice(start, this.index), this.label, this.line);
  }

  parseMapping() {
    const result = Object.create(null);
    this.index += 1;
    this.skipSpace();
    if (this.input[this.index] === "}") {
      this.index += 1;
      return result;
    }
    while (this.index < this.input.length) {
      const key = this.parseFlowKey();
      this.skipSpace();
      if (this.input[this.index] !== ":") {
        this.error("flow mapping entry is missing ':'");
      }
      this.index += 1;
      if (Object.hasOwn(result, key)) {
        this.error(`duplicate mapping key ${JSON.stringify(key)}`);
      }
      result[key] = this.parseValue();
      this.skipSpace();
      const delimiter = this.input[this.index] ?? "";
      if (delimiter === "}") {
        this.index += 1;
        return result;
      }
      if (delimiter !== ",") {
        this.error("flow mapping entries must be comma-separated");
      }
      this.index += 1;
      this.skipSpace();
      if (this.input[this.index] === "}") {
        this.index += 1;
        return result;
      }
    }
    this.error("unterminated flow mapping");
  }

  parseSequence() {
    const result = [];
    this.index += 1;
    this.skipSpace();
    if (this.input[this.index] === "]") {
      this.index += 1;
      return result;
    }
    while (this.index < this.input.length) {
      result.push(this.parseValue());
      this.skipSpace();
      const delimiter = this.input[this.index] ?? "";
      if (delimiter === "]") {
        this.index += 1;
        return result;
      }
      if (delimiter !== ",") {
        this.error("flow sequence entries must be comma-separated");
      }
      this.index += 1;
      this.skipSpace();
      if (this.input[this.index] === "]") {
        this.index += 1;
        return result;
      }
    }
    this.error("unterminated flow sequence");
  }
}

function parseInlineValue(value, label, line) {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    fail(label, line, "mapping value must be nonempty");
  }
  if (trimmed === "|" || trimmed === ">" || trimmed.startsWith("|-") || trimmed.startsWith(">-")) {
    fail(label, line, "block scalars are outside the lockfile profile");
  }
  if (forbiddenPlainScalarPrefixes.some((prefix) => trimmed.startsWith(prefix))) {
    fail(label, line, "anchors, aliases, and tags are forbidden");
  }
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    const parser = new FlowParser(trimmed, label, line);
    const result = parser.parseValue();
    parser.skipSpace();
    if (parser.index !== trimmed.length) {
      fail(label, line, "trailing content after flow value");
    }
    return result;
  }
  if (trimmed.startsWith("'") || trimmed.startsWith('"')) {
    return decodeQuotedScalar(trimmed, label, line);
  }
  return decodePlainScalar(trimmed, label, line);
}

function parseLines(lines, start, indent, label) {
  const sequence = lines[start]?.text === "-" || lines[start]?.text.startsWith("- ");
  const result = sequence ? [] : Object.create(null);
  let index = start;
  while (index < lines.length) {
    const line = lines[index];
    if (line.indent < indent) {
      break;
    }
    if (line.indent > indent) {
      fail(label, line.number, "unexpected indentation");
    }
    if (line.text === "?" || line.text.startsWith("? ")) {
      fail(label, line.number, "explicit mapping keys are outside the lockfile profile");
    }
    const lineIsSequence = line.text === "-" || line.text.startsWith("- ");
    if (lineIsSequence !== sequence) {
      fail(label, line.number, "cannot mix mapping and sequence entries at one indentation");
    }
    if (sequence) {
      const remainder = line.text.slice(1).trimStart();
      if (remainder.length > 0) {
        result.push(parseInlineValue(remainder, label, line.number));
        index += 1;
      } else {
        const next = lines[index + 1];
        if (next === undefined || next.indent <= indent) {
          result.push(null);
          index += 1;
        } else {
          const parsed = parseLines(lines, index + 1, next.indent, label);
          result.push(parsed.value);
          index = parsed.index;
        }
      }
      continue;
    }

    const colon = findMappingColon(line.text);
    if (colon < 0) {
      fail(label, line.number, "mapping entry is missing ':'");
    }
    const key = parseKey(line.text.slice(0, colon), label, line.number);
    if (Object.hasOwn(result, key)) {
      fail(label, line.number, `duplicate mapping key ${JSON.stringify(key)}`);
    }
    const remainder = line.text.slice(colon + 1).trimStart();
    if (remainder.length > 0) {
      result[key] = parseInlineValue(remainder, label, line.number);
      index += 1;
      continue;
    }
    const next = lines[index + 1];
    if (next === undefined || next.indent <= indent) {
      result[key] = null;
      index += 1;
    } else {
      const parsed = parseLines(lines, index + 1, next.indent, label);
      result[key] = parsed.value;
      index = parsed.index;
    }
  }
  return { value: result, index };
}

/**
 * Parses the deliberately bounded YAML profile emitted by supported pnpm locks.
 * Unsupported YAML features fail closed instead of being normalized before the
 * dependency-source policy sees them.
 */
export function parsePnpmLockYaml(input, label = "pnpm-lock.yaml") {
  if (input.startsWith("\ufeff")) {
    fail(label, 1, "byte-order marks are forbidden");
  }
  const physicalLines = input.split(/\r?\n/u);
  const lines = [];
  for (let index = 0; index < physicalLines.length; index += 1) {
    const physical = physicalLines[index] ?? "";
    const leadingWhitespace = physical.match(/^[ \t]*/u)?.[0] ?? "";
    if (leadingWhitespace.includes("\t")) {
      fail(label, index + 1, "tabs cannot be used for indentation");
    }
    const indentation = leadingWhitespace.length;
    const text = stripComment(physical.slice(indentation), label, index + 1).trimEnd();
    if (text.trim().length === 0 || text.trim() === "---" || text.trim() === "...") {
      continue;
    }
    if (text.trimStart().startsWith("%")) {
      fail(label, index + 1, "directives are outside the lockfile profile");
    }
    lines.push({ indent: indentation, text, number: index + 1 });
  }
  if (lines.length === 0) {
    fail(label, undefined, "document must be nonempty");
  }
  if (lines[0].indent !== 0) {
    fail(label, lines[0].number, "root mapping must start at indentation zero");
  }
  const parsed = parseLines(lines, 0, 0, label);
  if (parsed.index !== lines.length) {
    fail(label, lines[parsed.index]?.number, "unparsed document content");
  }
  if (typeof parsed.value !== "object" || parsed.value === null || Array.isArray(parsed.value)) {
    fail(label, lines[0].number, "root must be a mapping");
  }
  return parsed.value;
}
