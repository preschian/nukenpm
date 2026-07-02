# nukenpm website

Marketing site for [nukenpm](https://nukenpm.avalix.dev) — built with [Astro](https://astro.build) and deployed to Cloudflare Workers.

## Setup

Requires [Bun](https://bun.sh) and Node.js ≥ 22.12.

```bash
cd web
bun install
```

The site version is read automatically from `../cli/Cargo.toml` at build time.

## Commands

| Command | Action |
| :------ | :----- |
| `bun run dev` | Local dev server at `localhost:4321` |
| `bun run build` | Production build to `./dist/` |
| `bun run preview` | Preview the build with Wrangler |
| `bun run check` | Type-check with Astro |
| `bun run deploy` | Build and deploy to Cloudflare |

## Structure

```text
web/
├── public/          Static assets (favicon, OG image, headers)
├── src/
│   ├── components/  Astro components (Terminal demo, install box)
│   ├── config/      Site metadata
│   ├── layouts/     Base HTML shell + SEO
│   ├── pages/       Routes (index, 404)
│   └── scripts/     Client-side TypeScript
├── astro.config.mjs
└── wrangler.jsonc
```

## Deploy

```bash
bun run deploy
```

Requires Cloudflare credentials (`wrangler login` or `CLOUDFLARE_API_TOKEN` in CI).
