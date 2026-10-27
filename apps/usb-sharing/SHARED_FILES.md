# Shared files

The sharing Worker serves `/file/<R2 key>` from the `fujimod-shared-files` bucket.
Uploading an object there publishes it at that URL without a website deployment.
The bucket itself does not need a public R2 domain. There is no public upload API
or bucket listing. Administration uses authenticated Wrangler commands.

The viewer shows the file, its stored HTTP/custom metadata and a Download button.
Supported image and video MIME types use native media elements. `?type=download`
forces a download-only page; `?type=image` and `?type=video` cannot change the
object's media type. HTML, SVG and unknown types remain download-only. Metadata
is escaped as text, including attributes in the initial Open Graph markup.

`/media/<key>` streams allowlisted media with a controlled MIME type, `nosniff`,
a restrictive CSP and single byte-range support. `/download/<key>` always uses
an attachment disposition and `application/octet-stream`. Stored upload headers
are displayed but never replayed as response policy. These routes read only the
shared-files bucket, not the report/contribution bucket.

Image pages include Open Graph image metadata. For video previews in Discord,
upload a PNG poster at `<video key>.poster.png`. The page then includes
`og:type=video.other`, a direct HTTPS `og:video` URL and the poster's `og:image`.
The initial HTML contains all tags; no browser JavaScript is needed.

## Publish a file

From this directory, after signing in with `cf auth login`:

```sh
node ../../scripts/cloudflare/wrangler.ts r2 object put fujimod-shared-files/clips/demo.mp4 --remote --file /path/to/demo.mp4 --content-type video/mp4
node ../../scripts/cloudflare/wrangler.ts r2 object put fujimod-shared-files/clips/demo.mp4.poster.png --remote --file /path/to/poster.png --content-type image/png
```

Share `https://share.fujimod.com/file/clips/demo.mp4?type=video`.

## Contribution links

Completed contribution and issue-report uploads receive a separate read-only
capability at `/contribution/<token>`. The `publications` D1 table maps it to an
explicit list of uploaded objects. The upload/resume ID is never used as a share
token. A share URL lists its files, and each file has the same metadata/download
view. A recipient cannot upload, resume, list other submissions or select an
arbitrary object key. Downloads from these links are not cached. Deleting the
publication row revokes the link.

The website copies this URL after a successful upload and announces “Share URL
copied.” If clipboard access fails, the completed upload remains successful;
the dialog offers the URL and a Copy URL button.

## Deploy and test

Create `fujimod-shared-files` once with the authenticated Wrangler wrapper's
`r2 bucket create` command. Apply D1 migrations before deploying:

```sh
node ../../scripts/cloudflare/wrangler.ts d1 migrations apply fujimod-reports --remote
npm test
npm run check
npm run deploy
```

Worker tests exercise local R2 and D1, including media ranges, metadata escaping,
active-document downloads, separate private storage and read-only share links.
