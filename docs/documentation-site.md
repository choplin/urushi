# Documentation site

The public documentation site is Urushi's user-facing guide and reference. Its
source lives under `website/`, where Astro and Starlight turn the authored
content into a static site. Cloudflare serves that output as a Workers Static
Assets Worker. The site uses the project-local Cloudflare CLI (`cf`); Wrangler
is not part of the build or deployment path.

The production target is `https://urushi.choplin.dev`. The Worker owns that
custom domain directly; its `workers.dev` endpoint is disabled so there is one
canonical public origin.

## Documentation boundary

The repository README is the short entry point: it links to the published site
and gives contributors the ordinary commands for working on it. The website
owns the complete user documentation, navigation, examples, and conceptual
guides under `website/src/content/docs/`. Developer-facing architecture and
design records remain under `docs/`.

API reference links point to the versioned documentation hosted by docs.rs.
The website does not generate or publish a second rustdoc copy. This keeps the
guide and API reference on their native publication paths while allowing guide
pages to link directly to the relevant crate, module, type, or method.

## Build boundary

The website requires Node.js 22.19 or newer and the pnpm version declared in
`website/package.json`. Install its dependencies and build from the repository
root:

```sh
pnpm --dir website install --frozen-lockfile
pnpm --dir website build
```

The build runs Astro diagnostics, then `cf build` delegates to Astro. The final
Astro integration copies the complete `dist` tree—including Starlight's
Pagefind index—into Cloudflare Build Output under
`website/.cloudflare/output/v0/`. Keeping this adapter after the Starlight
integration is required because Starlight completes the search index in its
own final build hook.

The Worker name, compatibility date, and static-asset behavior have one shared
source in `website/scripts/cloudflare-worker.mjs`. Both
`website/cloudflare.config.ts` and the Build Output adapter consume it. Build
Output and generated site files are ignored by Git.

Before any production upload, validate the already-built artifact without API
mutation:

```sh
pnpm --dir website exec cf deploy --prebuilt --mode production --dry-run
```

The dry run must report the complete asset set and finish successfully.

## Production operation

`.github/workflows/deploy-docs.yml` is the primary production path. It runs on
a push to `main` only when a path under `website/` or the source brand icon
changes, and it can also be started manually with `workflow_dispatch` from
`main`. A manual run selected from another branch is skipped. Pull requests and
forks cannot trigger a production deployment. The workflow installs the pinned
Node.js, pnpm, and website dependencies, builds the site, and runs a dry-run
and production `cf deploy` against the same prebuilt artifact. External Actions
are pinned to full commit SHAs. Immediately before deployment, the workflow
compares the checked-out revision with the current `main` tip and rejects a
stale run so an older, slower run cannot roll production back.

Configure the GitHub Environment named `documentation-production` with a
required-reviewer deployment protection rule and these Environment secrets:

- `CLOUDFLARE_API_TOKEN`: an account-owned token with `Workers Scripts: Write`
  and `Account Settings: Read` for the target account, plus `Workers Routes:
  Write` limited to the `choplin.dev` zone.
- `CLOUDFLARE_ACCOUNT_ID`: the target Cloudflare account identifier.

The production credentials are exposed only to the final deploy step. The
workflow has read-only repository permissions, does not cancel an in-progress
production deployment, and serializes deployments through its concurrency
group. A failed build, dry-run, Environment review, or deploy is visible in the
workflow run and prevents later deployment steps from running.

For an authorized local recovery deployment, a maintainer can run this command
from a clean checkout of the integrated `main` revision:

```sh
pnpm --dir website deploy
```

The command rebuilds and validates the site, then deploys that exact prebuilt
artifact. It runs a dry-run and the production `cf deploy` consecutively
without rebuilding between them. Authenticate an interactive workstation with
`pnpm --dir website exec cf auth login`. Never write credentials or account
identifiers to repository files or logs.

Confirm success from both the `cf` deployment result and the public HTTPS URL.
Verify the home page, Introduction, Quickstart, View, Components, Canvas, a
direct nested-page request, loaded assets, navigation, and an unknown path that
must render the custom 404 page. Record the production URL and verification
result in the release work record.

`cf` is beta and pinned exactly in `website/package.json`. Upgrade it as a
reviewed dependency change: rebuild, compare the generated Build Output, and
repeat the dry run before using the new version for production.
