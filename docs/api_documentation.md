# API Documentation

SoroScope Core generates its own OpenAPI description from the route handlers
using [`utoipa`](https://docs.rs/utoipa). The description is produced from the
same Rust types the handlers use, so the published contract cannot drift away
from the implementation.

## Interactive Swagger UI

Start the server and open the documentation browser:

```bash
cargo run -p soroscope-core
# then visit:
#   http://localhost:8080/docs
```

`/docs` is the canonical entry point. `/swagger-ui` is kept as an alias so
existing bookmarks and deployment probes that point at the previous path keep
working; both serve the same document.

The raw specification is served as JSON at:

```
http://localhost:8080/api-docs/openapi.json
```

## Generating the specification file

Client SDKs are usually generated in CI, where booting the server (and its
database and Redis dependencies) is inconvenient. The `openapi` subcommand
writes the document straight to disk instead:

```bash
# writes ./openapi.json
cargo run -p soroscope-core -- openapi

# or choose the destination
cargo run -p soroscope-core -- openapi --out dist/openapi.json
```

The command exits non-zero and logs the cause if serialization or the write
fails, so it is safe to use as a CI build step. The emitted document follows
OpenAPI 3.0.3, which is what `utoipa` 4 produces.

Once generated, the file can be fed to any standard generator, for example:

```bash
npx @openapitools/openapi-generator-cli generate \
  -i openapi.json -g typescript-fetch -o web/generated/api
```

## Documented endpoints

| Tag | Endpoints |
| --- | --- |
| Analysis | `/analyze`, `/analyze/wasm`, `/analyze/wasm/branches`, `/analyze/optimize-limits`, `/analyze/compare`, `/analyze/gas-golfing` |
| Auth | `/auth/challenge`, `/auth/verify`, `/auth/emergency-pause`, `/auth/jwks` |
| Fee Market | `/fees/recommend`, `/fees/history`, `/fees/analytics` |
| Operations | `/health`, `/healthz`, `/readyz`, `/metrics`, `/api/v1/webhooks/incoming` |
| Analysis (batch) | `/api/v1/contracts/batch-state` |

The `/ws/jobs/:job_id` WebSocket upgrade and the `/graphql` endpoint are not
part of the REST description; WebSocket and GraphQL are outside the scope of
an OpenAPI document and are documented separately.

## Adding a new endpoint

1. Annotate the handler with `#[utoipa::path(...)]`, giving it a `tag`, a
   `path` that matches the router registration, and a `responses(...)` entry
   for every status code it can return.
2. Derive `ToSchema` on any request or response type the annotation mentions.
3. Register the handler in the `paths(...)` list of the `ApiDoc` derive, and
   any new types in `components(schemas(...))`.

A handler that is annotated but not listed in `paths(...)` silently stays out
of the document, so step 3 is the one worth double-checking.
