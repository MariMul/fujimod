#ifndef CAMERA_WASM_H
#define CAMERA_WASM_H
#include <stdint.h>
#include <stddef.h>

/* ABI 1: one serialized owner per worker. No concurrent/reentrant calls.
 * Handles are nonzero, monotonically allocated and invalid after destroy.
 * Unless stated otherwise, calls return 0 on success and -1 on error.
 * u64 parameters use JavaScript BigInt (Emscripten WASM_BIGINT).
 * All pointers belong to this module's linear memory. Output structs must be
 * aligned to 8 bytes. Upload spans are borrowed only for the duration of a call.
 * Views remain valid until the next mutating call for their session or destroy;
 * error(0) remains valid until the next create/destroy/invalid-handle operation.
 * Reacquire JS heap views after any call that may grow WebAssembly memory.
 */
typedef uint32_t CameraHandle;
typedef struct { const uint8_t *data; uint32_t len; } CameraBytes;
typedef struct { uint64_t ticks, instructions; uint32_t failed; } CameraStatus;
typedef struct {
    const uint8_t *data;
    uint32_t len, width, height, output;
    uint64_t sequence;
} CameraFrame;

uint8_t *camera_alloc(uint32_t len);
void camera_free(uint8_t *data, uint32_t len);
/* Copies typed processor/body/scenario JSON and verified firmware. Returns zero
 * on failure; error(0) gives the creation error. No implicit files or Python. */
CameraHandle camera_create(const uint8_t *config, uint32_t config_len,
                           const uint8_t *firmware, uint32_t firmware_len);
int32_t camera_destroy(CameraHandle handle);
int32_t camera_run_until(CameraHandle handle, uint64_t tick);
int32_t camera_input(CameraHandle handle, const uint8_t *key, uint32_t key_len,
                     uint32_t pressed, uint64_t at);
int32_t camera_status(CameraHandle handle, CameraStatus *out);
/* Returns 1 with a zeroed descriptor if no frame has been published yet.
 * Repeated reads observe the latest frame without consuming or copying it.
 * Pixels are tightly packed RGBA8888, top row first. */
int32_t camera_frame(CameraHandle handle, uint32_t output, CameraFrame *out);
int32_t camera_error(CameraHandle handle, CameraBytes *out);
/* Session construction uses the checked shared graph. It returns an unexecuted
 * board so durable inputs can be restored before guest boot writes. */
CameraHandle camera_session_create(const uint8_t *json, uint32_t json_len,
                                   const uint8_t *dat, uint32_t dat_len);
/* Native prepared run: `document` is {document, clockSeconds2000?, preferredLanguages?},
 * where the nested document is the exact JSON text of {schema, prepared, browser}.
 * The update container and boot ROM are required; NAND and settings are optional
 * (zero length selects none). Inputs are copied before construction. */
CameraHandle camera_prepared_session_create(const uint8_t *document, uint32_t document_len,
                                           const uint8_t *update, uint32_t update_len,
                                           const uint8_t *boot_rom, uint32_t boot_rom_len,
                                           const uint8_t *nand, uint32_t nand_len,
                                           const uint8_t *settings, uint32_t settings_len);
typedef struct { CameraBytes json, bytes; } CameraResult;
/* {action,options} JSON. History uses same-owner checkpoint IDs; history returns
 * CBOR input-log bytes and replay accepts them in the upload span. Boundary
 * u64 values are decimal JSON strings. fingerprint returns the complete Session
 * identity as a compact SHA-256 result. See SESSION_ABI.md; no durable contexts. */
int32_t camera_command(CameraHandle, const uint8_t *json, uint32_t json_len,
                       const uint8_t *bytes, uint32_t bytes_len, CameraResult *out);
int32_t camera_scene_buffer(CameraHandle, uint32_t width, uint32_t height, CameraBytes *out);
int32_t camera_scene_commit(CameraHandle, uint32_t enabled);
/* Shared board/CPU policy: UTF-8 "interactive" or "exact", before sessions.
 * This selects the board-owned device executor as well as CPU accounting. */
int32_t camera_policy_setup(const uint8_t *policy, uint32_t policy_len);
/* Configure the Rust-generated-code tier once before translation. Empty input
 * selects Rust defaults; JSON accepts true, false or a checked options object.
 * Errors use error(0). Stats are borrowed JSON until the next stats call. */
int32_t camera_jit_setup(const uint8_t *json, uint32_t json_len);
int32_t camera_jit_stats(CameraBytes *out);

#if defined(__wasm32__)
_Static_assert(sizeof(CameraResult) == 16, "CameraResult ABI");
_Static_assert(sizeof(CameraBytes) == 8, "CameraBytes ABI");
_Static_assert(sizeof(CameraStatus) == 24, "CameraStatus ABI");
_Static_assert(sizeof(CameraFrame) == 32, "CameraFrame ABI");
_Static_assert(offsetof(CameraFrame, sequence) == 24, "CameraFrame sequence ABI");
_Static_assert(offsetof(CameraStatus, instructions) == 8, "CameraStatus instructions ABI");
#endif
#endif
