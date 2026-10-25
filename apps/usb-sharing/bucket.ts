import type { Env } from './env.ts';

/** What an object in the private bucket belongs to. Every write records it in the D1 index. */
export interface IndexEntry {
  kind: 'issue-report' | 'contribution';
  id: string;
  /** One line a maintainer reads in a listing. */
  summary?: string;
  metadata?: Record<string, unknown>;
}

/** Records an object that is already in the bucket, such as an upload whose index write was interrupted. */
export async function indexExisting(env: Env, object: R2Object, entry: IndexEntry) {
  await index(env, object, entry);
}

async function index(env: Env, object: R2Object, entry: IndexEntry) {
  await env.REPORTS_INDEX.prepare(
    `INSERT INTO objects (key, kind, id, bytes, content_type, summary, metadata, stored_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
     ON CONFLICT (key) DO UPDATE SET kind = ?2, id = ?3, bytes = ?4, content_type = ?5,
       summary = ?6, metadata = ?7, stored_at = ?8`,
  )
    .bind(
      object.key,
      entry.kind,
      entry.id,
      object.size,
      object.httpMetadata?.contentType ?? null,
      entry.summary ?? null,
      JSON.stringify(entry.metadata ?? {}),
      object.uploaded.toISOString(),
    )
    .run();
}

/** The only way to store an object in the private bucket: R2, then its index row. */
export async function putObject(
  env: Env,
  key: string,
  body: Parameters<R2Bucket['put']>[1],
  entry: IndexEntry,
  options?: R2PutOptions,
) {
  const object = await env.ISSUE_REPORTS.put(key, body, options);
  if (!object) throw Error(`R2 did not store ${key}`);
  await index(env, object, entry);
  return object;
}

/** Completes a multipart upload into the private bucket, then writes its index row. */
export async function completeUpload(
  env: Env,
  upload: R2MultipartUpload,
  parts: R2UploadedPart[],
  entry: IndexEntry,
) {
  const object = await upload.complete(parts);
  await index(env, object, entry);
  return object;
}
