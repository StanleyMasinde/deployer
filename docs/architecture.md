# Architecture

Deployer keeps one Rust crate and a separate frontend toolchain. They form one
versioned application release. MySQL stores app configuration and deployment jobs;
no Redis, container engine, Node.js runtime, or build toolchain is required by the
application on the VPS.

## Module boundaries

| Module | Responsibility |
| --- | --- |
| `domain` | App and deployment data; input invariants |
| `application` | App registration, workflow requests, completed-build handling, root setup, sessions |
| `providers` | Native Rust future-based source provider contract |
| `providers/github` | Installation authentication, workflow PRs, verified webhooks, artifacts |
| `storage` | Toasty models, MySQL queries, queue state, embedded migrations |
| `publishing` | Bounded ZIP extraction, document-root switching, Nginx activation |
| `http` | Routes, authentication middleware, JSON:API transport, frontend serving |
| `worker` | Job processing, retries, recovery, graceful shutdown |

Application services have no Axum dependency and do not import HTTP state or
request types. Persistence models remain inside storage. Provider credentials live
under `[providers.github]`; the core deployment job identifies its provider without
storing GitHub payloads or temporary installation tokens.

## Database tables

`models.rs` contains only the Toasty definitions for `User`, `App`, `Deployment`,
and `Job`, producing `users`, `apps`, `deployments`, and `jobs`. Conversion code
lives in `storage/mappings.rs`; database operations live in their respective storage
modules.

`users` holds the root account, including its email address. `apps` holds
repository and publishing settings. `deployments` records the source run, commit,
deployment status, error, and timestamps.

`jobs` holds worker status, attempt count, and scheduling or claim deadlines.
Deployment jobs uniquely reference a deployment. Email jobs have no deployment
reference and carry a rendered message in their payload. Both use the same table,
claims, retries, and recovery; terminal jobs discard their message payload.
Enqueuing, claiming, retrying, recovering, and completing deployment work update
both tables transactionally.

## First-run setup

The web process checks MySQL for the singleton root account. Until it exists,
`/api/setup` reports that setup is required and the UI guides the owner through
account creation. A private, one-time code in `server.state_root` authorizes that
operation. The fixed root account primary key prevents a second account from being
created concurrently. Password hashing belongs to the application layer; configuration
contains no administrator password or password hash.

## Frontend serving

Vue Router generates routes from `frontend/src/pages/` using its Vite plugin.
`definePage` metadata opts public routes into prerendering. ViteSSG writes the public
HTML, while a build plugin preserves Vite’s empty application shell as `spa.html`.
Console routes render on the client and load private data only through authenticated
API calls. Missing JS or CSS files return 404 instead of the SPA shell.

Debug Rust executables start or reuse Vite and proxy frontend requests. Release
executables require both HTML files under `assets_root/CARGO_VERSION/`. A wrong or
missing frontend version stops startup rather than serving a mismatched UI.

## Deployment jobs

The HTTP process validates the webhook signature and normalizes eligible events.
The database uniquely identifies a job by provider, app, run, and attempt. The
worker fetches run metadata from the provider again and verifies its branch, SHA,
workflow, event, origin repository, and attempt before selecting its artifact.

Downloads stream to disk with a compressed-byte limit. Extraction rejects traversal,
links, special files, excessive file counts, and oversized output. The artifact must
contain `index.html` at its root. The worker publishes one job at a time and locks
the apps directory to prevent a second worker from using it.

The worker atomically replaces a relative `current` symlink. Previous content is
held only while Nginx activation is pending, then removed. An activation failure
restores the prior link. A crash can leave a queued retry or orphaned content;
startup recovers interrupted jobs and publication cleans temporary files. A ten-minute
claim deadline also recovers jobs after transient database failures without a restart. An older
run cannot overwrite a newer completed deployment.

## Nginx permissions

The installer validates `deploy/deployer.sudoers` with `visudo` and installs it as
`/etc/sudoers.d/deployer`; it never edits `/etc/sudoers`. Removing the dedicated
file revokes helper access.

The web service disables privilege escalation. The worker can call a root-owned helper
whose arguments are a numeric app ID, a validated domain, and a static/SPA mode.
The helper constructs its own configuration with a fixed document-root prefix;
it never accepts Nginx configuration supplied by the worker. It validates and
reloads Nginx, restoring its configuration on failure. Existing app configurations
are preserved so operator-managed TLS settings survive deployments.

## Add a provider

Implement `SourceProvider` with native Rust futures and `Send` bounds. Add provider
configuration under `[providers.PROVIDER]` and translate its authenticated completion
event into `BuildCompleted`. Keep authentication, payloads, workflow generation,
and artifact discovery inside the provider adapter. Route jobs to the corresponding
adapter through `providers::current_provider`, selected by `[providers].current`.
The worker and web server use that factory; adapter construction stays in the
provider module. GitHub is the supported MVP selection.

## Change the schema

Update the Toasty models, then use the migration CLI against a development database:

```sh
cargo run --bin migrate -- migration generate
```

Commit the migration, history entry, and snapshot together. To apply migrations
manually, run:

```sh
cargo run --bin migrate -- migration apply
```

The migration binary loads only `[database]`, connects directly to Toasty, and
delegates commands to `ToastyCli`. It does not require server, provider, or worker
configuration. Migration embedding lives in the web executable, so the migration
CLI can compile and generate the initial migration from an empty `toasty/` directory.
Configure `database.test_url` and prepend `--test` to target a
separate test database:

```sh
cargo run --bin migrate -- --test migration apply
```

The web server applies embedded migrations on every boot before accepting requests.
Its systemd readiness check gates worker startup. The worker does not run migrations.

## Email delivery

`mail/templates.rs` renders plain-text and HTML messages using Askama templates.
`mail::QueuedMailer` persists rendered messages in `jobs`; SMTP credentials stay
in configuration. The worker dispatches each job by kind and uses the same claim,
attempt limit, and recovery path for email and deployments. SMTP 4xx failures are
retried; 5xx rejections fail permanently. A bounded delivery timeout fits within
the worker claim deadline. Each message uses a stable job-based Message-ID, but
SMTP delivery remains at least once.

Root-account creation saves the email address and queues the optional welcome
message in one database transaction. Configure outbound transport under `[mail]`
and `[mail.smtp]`. STARTTLS is required by default; implicit TLS is also available.
Unencrypted SMTP is limited to a local relay such as Mailpit. Completed and failed
email jobs retain status and a short error code while removing their payload.
