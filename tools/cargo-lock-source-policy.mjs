const bareValue = Symbol("toml-bare-value");

function fail(label, line, column, message) {
  throw new Error(`${label}:${line}:${column}: invalid Cargo.lock TOML: ${message}`);
}

function assertUnicodeScalar(codePoint, label, line, column) {
  if (codePoint > 0x10ffff || (codePoint >= 0xd800 && codePoint <= 0xdfff)) {
    fail(label, line, column, "escape does not encode a Unicode scalar value");
  }
  return String.fromCodePoint(codePoint);
}

class TomlParser {
  constructor(input, label) {
    this.input = input;
    this.label = label;
    this.index = 0;
    this.line = 1;
    this.column = 1;
    this.document = Object.create(null);
    this.current = this.document;
    this.declaredTables = new Set();
  }

  error(message) {
    fail(this.label, this.line, this.column, message);
  }

  peek(offset = 0) {
    return this.input[this.index + offset] ?? "";
  }

  consume() {
    const character = this.peek();
    if (character === "") {
      return "";
    }
    this.index += 1;
    if (character === "\n") {
      this.line += 1;
      this.column = 1;
    } else {
      this.column += 1;
    }
    return character;
  }

  consumeExpected(expected) {
    if (!this.input.startsWith(expected, this.index)) {
      this.error(`expected ${JSON.stringify(expected)}`);
    }
    for (let index = 0; index < expected.length; index += 1) {
      this.consume();
    }
  }

  skipHorizontalSpace() {
    while (this.peek() === " " || this.peek() === "\t") {
      this.consume();
    }
  }

  skipComment() {
    if (this.peek() !== "#") {
      return;
    }
    while (this.peek() !== "" && this.peek() !== "\n") {
      this.consume();
    }
  }

  skipDocumentTrivia() {
    while (true) {
      this.skipHorizontalSpace();
      this.skipComment();
      if (this.peek() === "\r" && this.peek(1) === "\n") {
        this.consume();
        this.consume();
      } else if (this.peek() === "\n") {
        this.consume();
      } else {
        return;
      }
    }
  }

  skipValueTrivia() {
    while (true) {
      this.skipHorizontalSpace();
      this.skipComment();
      if (this.peek() === "\r" && this.peek(1) === "\n") {
        this.consume();
        this.consume();
      } else if (this.peek() === "\n") {
        this.consume();
      } else {
        return;
      }
    }
  }

  finishStatement() {
    this.skipHorizontalSpace();
    this.skipComment();
    if (this.peek() === "\r" && this.peek(1) === "\n") {
      this.consume();
      this.consume();
    } else if (this.peek() === "\n") {
      this.consume();
    } else if (this.peek() !== "") {
      this.error("unexpected content after statement");
    }
  }

  parseBareKey() {
    const start = this.index;
    while (/[A-Za-z0-9_-]/u.test(this.peek())) {
      this.consume();
    }
    if (this.index === start) {
      this.error("expected a bare or quoted key");
    }
    return this.input.slice(start, this.index);
  }

  parseKeyPart() {
    if (this.peek() === '"' || this.peek() === "'") {
      return this.parseString();
    }
    return this.parseBareKey();
  }

  parseKeyPath() {
    const parts = [];
    while (true) {
      this.skipHorizontalSpace();
      parts.push(this.parseKeyPart());
      this.skipHorizontalSpace();
      if (this.peek() !== ".") {
        return parts;
      }
      this.consume();
    }
  }

  parseEscape() {
    const escapeLine = this.line;
    const escapeColumn = this.column;
    const escape = this.consume();
    const simple = new Map([
      ["b", "\b"],
      ["t", "\t"],
      ["n", "\n"],
      ["f", "\f"],
      ["r", "\r"],
      ['"', '"'],
      ["\\", "\\"],
    ]);
    if (simple.has(escape)) {
      return simple.get(escape);
    }
    const digits = escape === "u" ? 4 : escape === "U" ? 8 : 0;
    if (digits === 0) {
      fail(this.label, escapeLine, escapeColumn, `unknown string escape \\${escape}`);
    }
    let hex = "";
    for (let index = 0; index < digits; index += 1) {
      const character = this.consume();
      if (!/[0-9a-f]/iu.test(character)) {
        fail(this.label, escapeLine, escapeColumn, `invalid \\${escape} Unicode escape`);
      }
      hex += character;
    }
    return assertUnicodeScalar(
      Number.parseInt(hex, 16),
      this.label,
      escapeLine,
      escapeColumn,
    );
  }

  parseBasicString(multiline) {
    this.consumeExpected(multiline ? '"""' : '"');
    if (multiline) {
      if (this.peek() === "\r" && this.peek(1) === "\n") {
        this.consume();
        this.consume();
      } else if (this.peek() === "\n") {
        this.consume();
      }
    }
    let result = "";
    while (this.peek() !== "") {
      if (multiline && this.input.startsWith('"""', this.index)) {
        this.consumeExpected('"""');
        return result;
      }
      const character = this.consume();
      if (!multiline && (character === "\n" || character === "\r")) {
        this.error("basic string cannot contain a newline");
      }
      if (!multiline && character === '"') {
        return result;
      }
      if (character === "\\") {
        if (multiline) {
          const continuationIndex = this.index;
          const continuationColumn = this.column;
          while (this.peek() === " " || this.peek() === "\t") {
            this.consume();
          }
          if (this.peek() === "\r" && this.peek(1) === "\n") {
            this.consume();
            this.consume();
            while (/\s/u.test(this.peek())) {
              this.consume();
            }
            continue;
          }
          if (this.peek() === "\n") {
            this.consume();
            while (/\s/u.test(this.peek())) {
              this.consume();
            }
            continue;
          }
          this.index = continuationIndex;
          this.column = continuationColumn;
        }
        result += this.parseEscape();
      } else {
        const codeUnit = character.charCodeAt(0);
        if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
          const next = this.peek().charCodeAt(0);
          if (!(next >= 0xdc00 && next <= 0xdfff)) {
            this.error("string contains an unpaired high surrogate");
          }
          result += character + this.consume();
          continue;
        }
        if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
          this.error("string contains an unpaired low surrogate");
        }
        if (codeUnit < 0x20 && character !== "\n" && character !== "\t") {
          this.error("string contains a forbidden control character");
        }
        result += character;
      }
    }
    this.error("unterminated basic string");
  }

  parseLiteralString(multiline) {
    this.consumeExpected(multiline ? "'''" : "'");
    if (multiline) {
      if (this.peek() === "\r" && this.peek(1) === "\n") {
        this.consume();
        this.consume();
      } else if (this.peek() === "\n") {
        this.consume();
      }
    }
    let result = "";
    while (this.peek() !== "") {
      if (multiline && this.input.startsWith("'''", this.index)) {
        this.consumeExpected("'''");
        return result;
      }
      const character = this.consume();
      if (!multiline && (character === "\n" || character === "\r")) {
        this.error("literal string cannot contain a newline");
      }
      if (!multiline && character === "'") {
        return result;
      }
      const codeUnit = character.charCodeAt(0);
      if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
        const next = this.peek().charCodeAt(0);
        if (!(next >= 0xdc00 && next <= 0xdfff)) {
          this.error("string contains an unpaired high surrogate");
        }
        result += character + this.consume();
        continue;
      }
      if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
        this.error("string contains an unpaired low surrogate");
      }
      if (codeUnit < 0x20 && character !== "\n" && character !== "\t") {
        this.error("string contains a forbidden control character");
      }
      result += character;
    }
    this.error("unterminated literal string");
  }

  parseString() {
    if (this.input.startsWith('"""', this.index)) {
      return this.parseBasicString(true);
    }
    if (this.input.startsWith("'''", this.index)) {
      return this.parseLiteralString(true);
    }
    if (this.peek() === '"') {
      return this.parseBasicString(false);
    }
    if (this.peek() === "'") {
      return this.parseLiteralString(false);
    }
    this.error("expected a TOML string");
  }

  parseBareValue() {
    const start = this.index;
    while (
      this.peek() !== "" &&
      !/[\s,#\]\}]/u.test(this.peek())
    ) {
      this.consume();
    }
    const raw = this.input.slice(start, this.index);
    if (raw.length === 0) {
      this.error("expected a value");
    }
    const boolean = /^(?:true|false)$/u.test(raw);
    const number = /^[+-]?(?:(?:0|[1-9](?:_?[0-9])*)|0x[0-9a-f](?:_?[0-9a-f])*|0o[0-7](?:_?[0-7])*|0b[01](?:_?[01])*)(?:\.[0-9](?:_?[0-9])*)?(?:[eE][+-]?[0-9](?:_?[0-9])*)?$/iu.test(raw);
    const specialFloat = /^[+-]?(?:inf|nan)$/u.test(raw);
    const dateTime = /^\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[Zz]|[+-]\d{2}:\d{2})?)?$/u.test(raw)
      || /^\d{2}:\d{2}:\d{2}(?:\.\d+)?$/u.test(raw);
    if (!boolean && !number && !specialFloat && !dateTime) {
      this.error(`invalid bare value ${JSON.stringify(raw)}`);
    }
    return { [bareValue]: true, raw };
  }

  parseArray() {
    const result = [];
    this.consumeExpected("[");
    this.skipValueTrivia();
    if (this.peek() === "]") {
      this.consume();
      return result;
    }
    while (true) {
      result.push(this.parseValue());
      this.skipValueTrivia();
      if (this.peek() === "]") {
        this.consume();
        return result;
      }
      if (this.peek() !== ",") {
        this.error("array entries must be comma-separated");
      }
      this.consume();
      this.skipValueTrivia();
      if (this.peek() === "]") {
        this.consume();
        return result;
      }
    }
  }

  assign(object, path, value) {
    let target = object;
    for (const part of path.slice(0, -1)) {
      if (!Object.hasOwn(target, part)) {
        target[part] = Object.create(null);
      }
      if (
        typeof target[part] !== "object" ||
        target[part] === null ||
        Array.isArray(target[part]) ||
        target[part][bareValue]
      ) {
        this.error(`dotted key ${path.join(".")} conflicts with an existing value`);
      }
      target = target[part];
    }
    const final = path.at(-1);
    if (final === undefined || Object.hasOwn(target, final)) {
      this.error(`duplicate key ${path.join(".")}`);
    }
    target[final] = value;
  }

  parseInlineTable() {
    const result = Object.create(null);
    this.consumeExpected("{");
    this.skipHorizontalSpace();
    if (this.peek() === "}") {
      this.consume();
      return result;
    }
    while (true) {
      const path = this.parseKeyPath();
      this.skipHorizontalSpace();
      this.consumeExpected("=");
      this.skipHorizontalSpace();
      this.assign(result, path, this.parseValue());
      this.skipHorizontalSpace();
      if (this.peek() === "}") {
        this.consume();
        return result;
      }
      if (this.peek() !== ",") {
        this.error("inline-table entries must be comma-separated");
      }
      this.consume();
      this.skipHorizontalSpace();
      if (this.peek() === "}") {
        this.error("inline tables cannot have a trailing comma");
      }
    }
  }

  parseValue() {
    const character = this.peek();
    if (character === '"' || character === "'") {
      return this.parseString();
    }
    if (character === "[") {
      return this.parseArray();
    }
    if (character === "{") {
      return this.parseInlineTable();
    }
    return this.parseBareValue();
  }

  openTable(path, array) {
    const tableIdentity = path.join("\u0000");
    let target = this.document;
    for (const part of path.slice(0, -1)) {
      if (!Object.hasOwn(target, part)) {
        target[part] = Object.create(null);
      }
      const value = target[part];
      if (Array.isArray(value)) {
        const last = value.at(-1);
        if (last === undefined) {
          this.error(`table ${path.join(".")} has no current array element`);
        }
        target = last;
      } else if (typeof value === "object" && value !== null && !value[bareValue]) {
        target = value;
      } else {
        this.error(`table ${path.join(".")} conflicts with an existing value`);
      }
    }
    const final = path.at(-1);
    if (final === undefined) {
      this.error("table path must be nonempty");
    }
    if (array) {
      if (!Object.hasOwn(target, final)) {
        target[final] = [];
      }
      if (!Array.isArray(target[final])) {
        this.error(`array table ${path.join(".")} conflicts with an existing value`);
      }
      const entry = Object.create(null);
      target[final].push(entry);
      this.current = entry;
      return;
    }
    if (this.declaredTables.has(tableIdentity)) {
      this.error(`duplicate table declaration ${path.join(".")}`);
    }
    this.declaredTables.add(tableIdentity);
    if (!Object.hasOwn(target, final)) {
      target[final] = Object.create(null);
    }
    if (
      typeof target[final] !== "object" ||
      target[final] === null ||
      Array.isArray(target[final]) ||
      target[final][bareValue]
    ) {
      this.error(`table ${path.join(".")} conflicts with an existing value`);
    }
    this.current = target[final];
  }

  parseHeader() {
    const array = this.input.startsWith("[[", this.index);
    this.consumeExpected(array ? "[[" : "[");
    const path = this.parseKeyPath();
    this.skipHorizontalSpace();
    this.consumeExpected(array ? "]]" : "]");
    this.finishStatement();
    this.openTable(path, array);
  }

  parseAssignment() {
    const path = this.parseKeyPath();
    this.skipHorizontalSpace();
    this.consumeExpected("=");
    this.skipHorizontalSpace();
    const value = this.parseValue();
    this.finishStatement();
    this.assign(this.current, path, value);
  }

  parse() {
    if (this.peek() === "\ufeff") {
      this.error("byte-order marks are forbidden");
    }
    while (true) {
      this.skipDocumentTrivia();
      if (this.peek() === "") {
        return this.document;
      }
      if (this.peek() === "[") {
        this.parseHeader();
      } else {
        this.parseAssignment();
      }
    }
  }
}

function requireRecord(value, label) {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    value[bareValue]
  ) {
    throw new Error(`${label}: every package entry must be a TOML table`);
  }
  return value;
}

/** Parse Cargo.lock structurally and reject every Cargo Git package source. */
export function assertCargoLockSourcePolicy(lockText, label = "Cargo.lock") {
  const document = new TomlParser(lockText, label).parse();
  const packages = document.package;
  if (!Array.isArray(packages)) {
    throw new Error(`${label}: package must be an array of TOML tables`);
  }
  for (let index = 0; index < packages.length; index += 1) {
    const packageRecord = requireRecord(packages[index], `${label}.package[${index}]`);
    if (!Object.hasOwn(packageRecord, "source")) {
      continue;
    }
    const source = packageRecord.source;
    if (typeof source !== "string") {
      throw new Error(`${label}.package[${index}].source: source must be a TOML string`);
    }
    if (/^git\+/iu.test(source.trim())) {
      throw new Error(
        `${label}.package[${index}].source: Cargo Git dependencies are forbidden: ${JSON.stringify(source)}`,
      );
    }
  }
}
