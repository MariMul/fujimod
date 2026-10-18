# Camera issue reports

`POST /issue-reports` accepts an explicitly selected TAR, its QR descriptor and a
reporter's description. The browser never sends the URL fragment during page load.
The description becomes public in `justjake/fujimod`; the archive remains private
in R2. No GitHub account or credential is needed in the browser.

Intake validates the complete TAR against the descriptor, limits uploads to 8 MiB
and 64 files, and accepts only configured origins. The per-IP limit is five
requests per minute; a Durable Object serializes submissions and caps intake at
50 reports per UTC day. Origin filtering is not authentication. These limits
bound initial abuse; monitor the budget before increasing it.

The client keeps a random idempotency key across retries. R2 receives
`<id>/report.tar` and `<id>/metadata.json` before GitHub is contacted. A successful
response is a durable storage acknowledgement, so the browser can then request
camera deletion. An ambiguous GitHub response returns 202 with the report ID,
retains the archive and never automatically repeats the GitHub request. Retrieve
that ID from the private bucket to reconcile it manually. There is no public
archive download endpoint. Records and archives persist until maintainers delete
them; there is no automatic retention policy yet.

## Deployment

Use a Cloudflare credential authorized for this Worker, Durable Objects and R2.
Create `fujimod-issue-reports` as a private R2 bucket (do not enable r2.dev/public
access). Configure `GITHUB_ISSUES_TOKEN` with Issues: write access to
`justjake/fujimod` under the account that should author the issues, then deploy:

```sh
npx wrangler r2 bucket create fujimod-issue-reports
npx wrangler secret put GITHUB_ISSUES_TOKEN
npm run deploy
```

Never reuse a developer's local GitHub credential implicitly. The token stays in
Worker secrets. The Wrangler v2 migration adds the `IssueReports` Durable Object;
existing USB sharing objects and TURN secrets remain intact.

Maintainers can retrieve a report with their Cloudflare credentials:

```sh
npx wrangler r2 object get fujimod-issue-reports/REPORT_ID/report.tar --remote --file report.tar
```

Run `node --test worker.test.mjs issue-reports.test.mjs`. Tests use a real local
Worker/R2/Durable Object runtime and intercept GitHub; no actual issues are posted.
