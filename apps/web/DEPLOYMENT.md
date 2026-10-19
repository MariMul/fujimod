# Production hosting

The website is published at `fujimod.com` and `www.fujimod.com` (owner request,
2026-10-19). The `fujimod-web` Worker serves the static build from
`.github/workflows/cloudflare.yml`; its first deployment, run
[39289474235](https://github.com/justjake/fujimod/actions/runs/39289474235),
moved both custom domains from the `apps/domain-endpoint` placeholder Worker,
which no longer serves them and can be retired. The separate
[USB sharing Worker](../usb-sharing/README.md) serves `https://share.fujimod.com`;
its WebSocket endpoint is `wss://share.fujimod.com/usb-proxy/signal`.

Gandi remains the registrar. Cloudflare hosts the entire DNS zone, delegated to
`ali.ns.cloudflare.com` and `rudy.ns.cloudflare.com`. Mail, SPF, DKIM, webmail and
mail service-discovery records were imported from Gandi before delegation changed.
Worker custom domains manage the website's DNS records and HTTPS certificates.
GitHub Pages is disabled.

The sharing Worker currently has no TURN secrets and provides STUN only. TURN
usage monitoring and a budget cutoff must be implemented before enabling relay
credentials; the existing issuance quota is not a spending cap.

Every deployment stages the newest published release's assets, including the
homepage's feature media (`docs-media.json`). A release without that inventory
deploys a homepage without preview clips, so build each release with
`tools/build_patch_release.py --docs-media`.

## Releases

`.github/workflows/cloudflare.yml` builds main, checks the application, stages
verified published release artifacts, and deploys `dist/` with `wrangler.jsonc`.
It runs when a release is published or is manually dispatched on main. Review
branches cannot deploy production. GitHub Pages is no longer a deployment target.

The GitHub repository secret `CLOUDFLARE_API_TOKEN` must grant Workers Scripts
Write for this account and Workers Routes Write / Zone Read for this zone.
Cloudflare must permit the token to manage the configured custom domains.
Cloudflare account email verification is required for both Worker deployment
and deployment-token creation. Do not store a personal CLI OAuth token in CI.

For a manual deployment, stage release artifacts and build with:

```sh
npm run prepare:release
FUJIMOD_SIGNAL_URL=wss://share.fujimod.com/usb-proxy/signal npm run build
npx wrangler deploy
```

Use Wrangler sign-in or a deployment API token. `cf` keeps its OAuth credentials
separately from Wrangler. No Cloudflare secret belongs in the static build.
Build only committed source and approved release artifacts; local simulator
bundles, firmware inputs and unreleased mod builds are not production assets.

## Verify

Check HTTPS at both web hostnames, the Dev Tools route `/dev-tools/`, and the
sharing Worker's `/health` endpoint. Compare deployed static asset hashes with
the build. Verify MX, SPF, DKIM and service records after DNS changes.

Keep GitHub Pages available until the Cloudflare deployment and its certificates
have been verified, then disable Pages in the repository settings. DNS caches may
continue using the former nameservers until their delegation TTL expires.
