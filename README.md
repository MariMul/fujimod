# FujiMod

Camera tools, emulation and firmware mods. The browser application lives in
[apps/web](apps/web/README.md), including its body definitions, patch
interpreter, runtime adapter and browser checks.

Use `mise install`, then work in `apps/web`:

```sh
cd apps/web
npm ci
npm test
npm run dev
```

The root `site-build`, `site-test` and `site-preview` mise tasks run from that
application directory. The Pages workflow builds `apps/web` from approved
main releases. Review branches do not deploy Pages.
