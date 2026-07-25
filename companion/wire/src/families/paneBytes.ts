import { z } from 'zod';

// pane.bytes.* — the raw PTY byte lane (cmux-style streaming).
//
// This is the high-fidelity sibling of the plain-text `pane.*` lane in pane.ts.
// Where `pane.text` ships a debounced, re-rendered viewport snapshot, this lane
// streams the terminal's raw output bytes verbatim so the phone can drive a real
// VT emulator. The phone reconstructs the screen locally; the Mac just forwards
// PTY bytes.
//
// Sequencing model
// ----------------
// - `epoch` is an opaque string minted once per *surface lifetime* on the Mac.
//   It changes whenever the underlying PTY/surface restarts. On any epoch change
//   the phone MUST discard its emulator state and cold-attach again — byte
//   offsets from the old epoch are meaningless against the new stream.
// - `seq` is the BYTE OFFSET of a chunk's first byte within its epoch. It starts
//   at `startSeq` (from `pane.bytes.attached`) and advances by the DECODED byte
//   length of each chunk's `data`. The next expected offset is therefore
//   `seq + decodedByteLength(data)`. The phone detects loss when an incoming
//   chunk's `seq` does not equal its expected offset, and recovers by
//   re-attaching with `lastSeq` set to that expected offset.
// - `lastSeq` is an EXCLUSIVE resume cursor: the first missing byte offset
//   (i.e. the phone's next expected offset after applying contiguous bytes
//   `[0, lastSeq)`). A non-truncated warm reply for the same epoch MUST use
//   `startSeq == lastSeq` and `replay` covering `[lastSeq, …)` through the
//   ring head. Do NOT treat `lastSeq` as the last *included* byte index.
//
// Base64
// ------
// All binary fields (`replay`, `data`) are STANDARD base64 (RFC 4648 §4,
// `+`/`/` alphabet, `=` padding) — matching the other envelope-layer binary
// payloads (`push` sealedPayload, pairing DER keys). NOT base64url; that flavor
// is reserved for the outer relay transport framing (see relay.ts). Empty string
// is a valid encoding of zero bytes (used for an empty `replay`).

/**
 * Max decoded byte length of a single `pane.bytes.chunk` payload before base64
 * encoding. Producers MUST split longer runs across multiple chunks.
 *
 * 32 KiB leaves generous headroom under the relay's 256 KiB frame cap
 * (`maxFrameBytes`, companion/relay/src/config.ts): the raw bytes are base64'd
 * (~×4/3) into the envelope payload, the envelope is E2E-sealed, and the sealed
 * blob is base64'd again (~×4/3) into the outer `relay.frame`. Even the doubled
 * expansion of a full 32 KiB chunk (~58 KiB) stays well inside the cap.
 */
export const PANE_BYTES_MAX_CHUNK_BYTES = 32 * 1024;

/**
 * Max base64 string length for a `chunk.data` field: `ceil(N/3)*4` for
 * N = {@link PANE_BYTES_MAX_CHUNK_BYTES}. Bounds the frame at the schema layer
 * without having to decode.
 */
export const PANE_BYTES_MAX_CHUNK_B64_LEN =
  Math.ceil(PANE_BYTES_MAX_CHUNK_BYTES / 3) * 4;

/** Standard-base64 body (RFC 4648 §4). Empty string = zero bytes. */
const Base64Std = z.string().regex(/^[A-Za-z0-9+/]*={0,2}$/, 'standard base64');

/**
 * `pane.bytes.attach` (phone → mac). Request half; the reply is
 * `pane.bytes.attached`, correlated via the envelope `replyTo` (same machinery
 * as `pane.scrollback`).
 *
 * - Cold attach: omit both `lastSeq` and `epoch`. The mac replays whatever tail
 *   it has and streams forward.
 * - Warm re-attach (after a detected gap): carry both `lastSeq` (exclusive
 *   resume cursor — first missing byte) and `epoch`. The mac resumes from
 *   `lastSeq` when the ring still covers it, else replies with `truncated: true`.
 *
 * Partial warm shapes (only `lastSeq` or only `epoch`) are rejected by the
 * schema — both fields are required together, or both absent.
 */
export const PaneBytesAttach = z
  .object({
    paneId: z.string(),
    lastSeq: z.number().int().gte(0).optional(),
    epoch: z.string().optional(),
  })
  .superRefine((value, ctx) => {
    const hasLast = value.lastSeq !== undefined;
    const hasEpoch = value.epoch !== undefined;
    if (hasLast !== hasEpoch) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'warm attach requires both lastSeq and epoch; cold attach omits both',
        path: hasLast ? ['epoch'] : ['lastSeq'],
      });
    }
  });

/**
 * `pane.bytes.attached` (mac → phone). Reply half of `pane.bytes.attach`,
 * carried with `replyTo` = the attach frame's envelope `id`.
 *
 * - `epoch`: the current surface epoch. If it differs from the phone's, the
 *   phone resets its emulator before applying `replay`.
 * - `startSeq`: the byte offset of the FIRST byte in `replay`. Subsequent
 *   `pane.bytes.chunk.seq` values continue from `startSeq + decoded(replay)`.
 * - `replay`: standard-base64 tail-ring bytes from `startSeq`. May be empty
 *   (`""`) — e.g. a cold attach to a surface with no buffered output yet.
 * - `truncated`: `true` when the ring could NOT cover the requested resume
 *   point (`lastSeq`/`epoch` fell off the back of the ring, or the epoch
 *   changed). The phone MUST reset its emulator before writing `replay`, since
 *   `replay` is then a fresh tail rather than a continuation.
 */
export const PaneBytesAttached = z.object({
  paneId: z.string(),
  epoch: z.string(),
  startSeq: z.number().int().gte(0),
  replay: Base64Std,
  truncated: z.boolean(),
});

/**
 * `pane.bytes.chunk` (mac → phone). One run of raw PTY output.
 *
 * - `epoch`: the surface epoch these bytes belong to. A chunk whose `epoch` the
 *   phone doesn't recognize means it missed an epoch change → cold-attach.
 * - `seq`: byte offset of `data`'s first byte within `epoch` (see sequencing
 *   model above). Loss is detected when `seq !=` the phone's expected offset.
 * - `data`: standard-base64 raw bytes, decoded length ≤
 *   {@link PANE_BYTES_MAX_CHUNK_BYTES}.
 */
export const PaneBytesChunk = z.object({
  paneId: z.string(),
  epoch: z.string(),
  seq: z.number().int().gte(0),
  data: Base64Std.max(PANE_BYTES_MAX_CHUNK_B64_LEN),
});

/** `pane.bytes.detach` (phone → mac). Stop streaming this pane's byte lane. */
export const PaneBytesDetach = z.object({
  paneId: z.string(),
});

export const paneBytesMessages = {
  'pane.bytes.attach': PaneBytesAttach,
  'pane.bytes.attached': PaneBytesAttached,
  'pane.bytes.chunk': PaneBytesChunk,
  'pane.bytes.detach': PaneBytesDetach,
} as const;
