import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import protobuf from "protobufjs";

const protoUrl = new URL(
  "../../../schemas/proto/academic/v1/local_core.proto",
  import.meta.url,
);
const expectedProtoSha256 = "bcba604f2656ab0ccf0788c981ad7212a09e284156066b9ae252cd9e35783d2f";
const requestFrameHex = "000000b61ab3010a10000102030405060708090a0b0c0d0e0f1210101112131415161718191a1b1c1d1e1f1a20202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f2220404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f28ffffffffffffffffff01322b6c6561726e696e672d706c6174666f726d2e6c6f63616c2e73796e7468657469632d696e676573742e763152110a0f7369676e65642d62617463682d7632";
const responseFrameHex = "000001062283020a10000102030405060708090a0b0c0d0e0f10011a084143434550544544229d010a10606162636465666768696a6b6c6d6e6f1210000102030405060708090a0b0c0d0e0f1a10101112131415161718191a1b1c1d1e1f2220202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f2a20404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f30ffffffffffffffffff013a1608feffffffffffffffff0110ffffffffffffffffff0128ffffffffffffffffff01321608feffffffffffffffff0110ffffffffffffffffff013a20808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";

interface DecodedRequestEnvelope {
  readonly mutable_request?: {
    readonly request_id?: readonly number[];
    readonly client_instance_id?: readonly number[];
    readonly idempotency_key?: readonly number[];
    readonly request_digest?: readonly number[];
    readonly expected_profile_revision?: string;
    readonly capability_id?: string;
    readonly command?: string;
    readonly synthetic_ingest?: {
      readonly synthetic_fixture_id?: string;
    };
  };
}

interface DecodedResponseEnvelope {
  readonly mutable_response?: {
    readonly request_id?: readonly number[];
    readonly status?: string;
    readonly reason?: string;
    readonly profile_revision?: string;
    readonly acceptance_range?: {
      readonly accept_seq_start?: string;
      readonly accept_seq_end?: string;
    };
    readonly response_digest?: readonly number[];
    readonly receipt?: {
      readonly receipt_id?: readonly number[];
      readonly request_id?: readonly number[];
      readonly client_instance_id?: readonly number[];
      readonly idempotency_key?: readonly number[];
      readonly request_digest?: readonly number[];
      readonly profile_revision?: string;
      readonly acceptance_range?: {
        readonly accept_seq_start?: string;
        readonly accept_seq_end?: string;
      };
    };
  };
}

function byteSequence(start: number, length: number): Buffer {
  return Buffer.from(Array.from({ length }, (_, index) => start + index));
}

function framed(type: protobuf.Type, value: Record<string, unknown>): Buffer {
  const message = type.fromObject(value);
  assert.equal(type.verify(message), null);
  const payload = Buffer.from(type.encode(message).finish());
  const prefix = Buffer.alloc(4);
  prefix.writeUInt32BE(payload.length);
  return Buffer.concat([prefix, payload]);
}

function decodeFramed(type: protobuf.Type, frame: Buffer): unknown {
  assert.equal(frame.readUInt32BE(0), frame.length - 4);
  return type.toObject(type.decode(frame.subarray(4)), {
    bytes: Array,
    enums: String,
    longs: String,
    oneofs: true,
  });
}

void test("local core Proto tags, oneofs, and reserved bands are drift-pinned", async () => {
  const protoBytes = await readFile(protoUrl);
  assert.equal(createHash("sha256").update(protoBytes).digest("hex"), expectedProtoSha256);
  const root = protobuf.parse(protoBytes.toString("utf8"), { keepCase: true }).root;
  const envelope = root.lookupType("academic.v1.LocalCoreEnvelope");
  const mutableRequest = root.lookupType("academic.v1.MutableRequest");

  assert.deepEqual(
    Object.fromEntries(Object.entries(envelope.fields).map(([name, field]) => [name, field.id])),
    { client_handshake: 1, server_handshake: 2, mutable_request: 3, mutable_response: 4 },
  );
  assert.deepEqual(envelope.oneofs.payload?.oneof, [
    "client_handshake",
    "server_handshake",
    "mutable_request",
    "mutable_response",
  ]);
  assert.deepEqual(envelope.reserved, [[5, 15]]);
  const commandOneof = mutableRequest.oneofs.command;
  assert.ok(commandOneof !== undefined);
  assert.deepEqual(commandOneof.oneof, [
    "synthetic_ingest",
    "synthetic_backup",
    "synthetic_restore",
  ]);
  assert.deepEqual(mutableRequest.reserved, [[7, 9], [13, 31]]);
  assert.deepEqual(
    Object.fromEntries(
      commandOneof.oneof.map((name) => {
        const field = mutableRequest.fields[name];
        assert.ok(field !== undefined);
        return [name, field.id];
      }),
    ),
    { synthetic_ingest: 10, synthetic_backup: 11, synthetic_restore: 12 },
  );
});

void test("TypeScript and Rust local-core request golden bytes match", async () => {
  const protoText = await readFile(protoUrl, "utf8");
  const root = protobuf.parse(protoText, { keepCase: true }).root;
  const envelope = root.lookupType("academic.v1.LocalCoreEnvelope");
  const frame = framed(envelope, {
    mutable_request: {
      request_id: byteSequence(0, 16),
      client_instance_id: byteSequence(16, 16),
      idempotency_key: byteSequence(32, 32),
      request_digest: byteSequence(64, 32),
      expected_profile_revision: "18446744073709551615",
      capability_id: "learning-platform.local.synthetic-ingest.v1",
      synthetic_ingest: { synthetic_fixture_id: "signed-batch-v2" },
    },
  });
  assert.equal(frame.toString("hex"), requestFrameHex);

  const decoded = decodeFramed(envelope, frame) as DecodedRequestEnvelope;
  const request = decoded.mutable_request;
  assert.ok(request !== undefined);
  assert.deepEqual(request.request_id, [...byteSequence(0, 16)]);
  assert.deepEqual(request.client_instance_id, [...byteSequence(16, 16)]);
  assert.deepEqual(request.idempotency_key, [...byteSequence(32, 32)]);
  assert.deepEqual(request.request_digest, [...byteSequence(64, 32)]);
  assert.equal(request.expected_profile_revision, "18446744073709551615");
  assert.equal(request.command, "synthetic_ingest");
  assert.equal(request.synthetic_ingest?.synthetic_fixture_id, "signed-batch-v2");
});

void test("TypeScript and Rust receipt golden preserves every u64 and byte field", async () => {
  const protoText = await readFile(protoUrl, "utf8");
  const root = protobuf.parse(protoText, { keepCase: true }).root;
  const envelope = root.lookupType("academic.v1.LocalCoreEnvelope");
  const acceptanceRange = {
    accept_seq_start: "18446744073709551614",
    accept_seq_end: "18446744073709551615",
  };
  const frame = framed(envelope, {
    mutable_response: {
      request_id: byteSequence(0, 16),
      status: 1,
      reason: "ACCEPTED",
      receipt: {
        receipt_id: byteSequence(96, 16),
        request_id: byteSequence(0, 16),
        client_instance_id: byteSequence(16, 16),
        idempotency_key: byteSequence(32, 32),
        request_digest: byteSequence(64, 32),
        profile_revision: "18446744073709551615",
        acceptance_range: acceptanceRange,
      },
      profile_revision: "18446744073709551615",
      acceptance_range: acceptanceRange,
      response_digest: byteSequence(128, 32),
    },
  });
  assert.equal(frame.toString("hex"), responseFrameHex);

  const decoded = decodeFramed(envelope, frame) as DecodedResponseEnvelope;
  const response = decoded.mutable_response;
  assert.ok(response !== undefined);
  assert.equal(response.status, "MUTATION_STATUS_ACCEPTED");
  assert.equal(response.reason, "ACCEPTED");
  assert.equal(response.profile_revision, "18446744073709551615");
  assert.deepEqual(response.acceptance_range, acceptanceRange);
  assert.deepEqual(response.response_digest, [...byteSequence(128, 32)]);
  const receipt = response.receipt;
  assert.ok(receipt !== undefined);
  assert.deepEqual(receipt.receipt_id, [...byteSequence(96, 16)]);
  assert.deepEqual(receipt.request_id, [...byteSequence(0, 16)]);
  assert.deepEqual(receipt.client_instance_id, [...byteSequence(16, 16)]);
  assert.deepEqual(receipt.idempotency_key, [...byteSequence(32, 32)]);
  assert.deepEqual(receipt.request_digest, [...byteSequence(64, 32)]);
  assert.equal(receipt.profile_revision, "18446744073709551615");
  assert.deepEqual(receipt.acceptance_range, acceptanceRange);
});
