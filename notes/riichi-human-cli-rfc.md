# Riichi human CLI RFC

**Status:** proposed
**Scope:** a human-friendly command-line client over the existing Riichi API
**Related:** [pilot PRD](./riichi-pilot-prd.md), [pilot architecture RFC](./riichi-pilot-architecture-rfc.md), [post-pilot product direction RFC](./riichi-post-pilot-product-direction-rfc.md)

## 1. Summary

Riichi already has a useful machine-facing CLI and MCP adapter, but both assume
the caller already has a project UUID, session UUID, and agent token. That is a
reasonable automation boundary and a poor human workflow.

Add a separate human-facing `riichi` CLI that resolves workspace context,
accepts issue keys, stores authentication securely, and presents readable
output. Keep `riichi-agent` as the thin automation/MCP adapter, but give it the
same named-profile and non-environment credential loading options. Both clients
must call the same server-authoritative operations; neither gets a second set
of dispatch or authorization rules.

## 2. Goals

- make common inspection and dispatch tasks usable without copying UUIDs;
- let a human select an organization, project, and agent session once;
- use issue keys such as `ENG-224` wherever the API can resolve them;
- provide concise terminal output by default and stable JSON with `--json`;
- preserve lease fencing, idempotency, approvals, and authorization on the API;
- make the safest common action easy while keeping authority-changing actions
  explicit;
- provide a path from one-off commands to a guided work loop.

## 3. Non-goals

- replacing the agent protocol with a second human-specific backend;
- making MCP a conversational or interactive human interface;
- storing agent tokens in shell history, project files, or plaintext command
  arguments by default;
- hiding lease state, approval requirements, or rejected operations behind
  friendly wording;
- building a complete terminal UI before the command model is proven.

## 4. Current problem

The current `riichi-agent` commands require all of this context for every run:

```bash
RIICHI_PROJECT_ID=... RIICHI_SESSION_ID=... RIICHI_AGENT_TOKEN=... riichi-agent ready
```

The CLI exposes `ready`, `claim`, `context`, and batch `report`. The API also
supports renewal, context resources, document operations, and quarantined
attempt reads, but those are not currently exposed by the CLI or MCP wrapper.

The result is technically safe but awkward for humans: identifiers are copied
from the browser, issue UUIDs are preferred over display keys, and output is
raw JSON even for simple status checks.

## 5. Proposed client boundary

### `riichi`

Human-facing CLI with local profiles, human authentication, issue-key
resolution, readable output, and guided workflows.

### `riichi-agent`

Machine-facing compatibility CLI and stdio MCP adapter. It keeps explicit
project/session/token inputs available for agents and CI. Its protocol surface
can grow independently without making the human CLI's UX carry machine-level
details.

Both clients use the same HTTP API and generated request/response types where
practical. The server remains authoritative for identity, project access,
leases, fencing, idempotency, approvals, and audit history.

## 6. Slices

### Slice 1: profiles and readable output

Add a local profile store with a selected API URL, organization, project, and
optional agent session. Human and agent commands resolve these values unless
explicitly overridden by flags. Agent bearer tokens live in the OS keychain or
an external credential file referenced explicitly at launch, never in the
profile's plain-text selection data.

Example:

```bash
riichi profile list
riichi project list
riichi project use riichi
riichi agent ready

# machine-friendly equivalent
riichi-agent --profile build-worker ready
```

Default output should be a compact table or status view. Every command that
supports structured output also accepts `--json`; JSON field names and shapes
must remain stable enough for scripts.

The profile store must use the platform configuration directory and restrictive
permissions. It may contain identifiers and preferences, but not bearer
tokens.

**Exit condition:** a user or a named worker profile can inspect the selected
project's ready queue without copying IDs into the shell.

### Slice 2: human authentication and secure credentials

Add `riichi login` using the existing human auth flow, through a browser
callback when a local browser is available or a device-code flow for headless
terminals. Store the resulting refresh or session credential in the OS
keychain. The normal human path should never require a credential environment
variable.

```bash
riichi login
riichi whoami
riichi logout
```

Human commands use the human API surface. They do not impersonate an agent
session. Agent commands continue to require an agent session and token. For
headless agent use, support an explicit `--token-stdin` or equivalent secure
input path. Keep environment variables as a compatibility option for CI and
existing automation, not as the recommended interactive workflow.

**Exit condition:** a human can authenticate once and run read-only commands
without tokens appearing in shell history or process listings.

### Slice 3: issue-key and resource resolution

Accept display keys in commands that currently require issue UUIDs:

```bash
riichi issue show ENG-224
riichi issue context ENG-224
riichi agent claim ENG-224
```

Resolution must be scoped to the selected organization/project or explicitly
report ambiguity. The CLI must not guess across projects when two keys could
match. The server should own the canonical lookup and authorization check;
client-side lookup is only a presentation optimization.

**Exit condition:** normal issue operations use the key a human sees in the UI,
while UUIDs remain available with `--id` or `--json`.

### Slice 4: explicit human work commands

Add a small set of readable commands over the existing operations:

```bash
riichi agent ready --limit 10
riichi agent claim ENG-224
riichi agent renew --lease ...
riichi agent report ENG-224 --release --comment "Needs API decision"
riichi agent report ENG-224 --complete --summary "Implemented and verified"
```

The CLI should translate friendly flags into the same bounded report batch
operations used by agents. It must print lease expiry, fencing-sensitive
errors, approval requirements, and idempotency outcomes rather than reducing
them to a generic failure.

Destructive or authority-changing commands require explicit action flags. A
confirmation prompt may be used interactively, but `--yes` must be available
for automation and must not bypass server authorization.

**Exit condition:** a human can perform the common claim, inspect, renew, and
report loop without constructing JSON manually.

### Slice 5: guided `work` loop

After the command model is stable, add:

```bash
riichi work
```

The loop should:

1. show eligible work and key dispatch facts;
2. let the human choose an issue;
3. claim it through the normal API;
4. show bounded context and active lease details;
5. offer explicit report, renew, or release actions.

This is a convenience flow, not a new authority layer. It must survive a
missed notification or interrupted terminal by refetching authoritative state.

**Exit condition:** a user can complete a routine dispatch task with a short,
discoverable terminal flow while still seeing the same state as the browser.

## 7. Command shape

Prefer nouns and verbs that match the product:

```text
riichi profile ...
riichi login|logout|whoami
riichi organization ...
riichi project ...
riichi session use ...
riichi issue ...
riichi agent ...
riichi work
```

Keep `riichi-agent ready|claim|context|report|mcp` as a compatibility surface
for existing generated commands and agent launch instructions. Do not silently
change its authentication or output contract while introducing `riichi`.

## 8. Security and correctness

- Human credentials belong in the OS keychain. Headless login uses device code;
  headless agent execution may read a token from stdin, a file descriptor, or
  an explicitly selected external secret manager/file.
- Agent tokens remain session-scoped and are never inferred from a human login.
- Profiles contain API and organization/project/session selection state, not
  bearer credentials or authority.
- Environment variables remain a compatibility path for CI, but the preferred
  machine path is `--profile`, with secret input separated from command
  arguments and process listings.
- Issue-key resolution is always authorization-aware and scope-aware.
- Claims, renewals, reports, and approvals remain versioned, fenced, and
  idempotent on the server.
- `--json` is a serialization mode, not a weaker validation path.
- Errors should include the server error code and the next safe action where
  one exists.

## 9. Rollout order

Implement slices in order: profiles/output, authentication, key resolution,
explicit work commands, then the guided loop. This gives us a useful CLI after
each slice and lets us validate command vocabulary before building a larger
interactive surface.

The first implementation target should be Slices 1 and 3 together if the
existing human session flow is not yet suitable for terminal login. That still
removes the UUID burden while keeping credentials supplied explicitly in
headless environments.

## 10. Current implementation slice

The first implementation slice now includes:

- `riichi` and `riichi-agent` binaries sharing the compatibility command
  surface;
- named agent profiles with separate restricted credential storage;
- token input through stdin and readable/JSON output modes;
- server-side agent issue-key resolution;
- renew, complete, release, and prompt-based `work` commands;
- browser-based CLI login handoff using the existing OIDC flow;
- separate human organization and project selection state.

The remaining verification work is end-to-end coverage of the browser handoff,
profile selection, and a real claim/report loop against a disposable database.

## 11. Open questions

The initial product decisions are:

1. Support both browser callback and device-code login. The CLI can choose the
   browser callback when it can open a local browser and fall back to device
   code for headless terminals.
2. Keep organization selection separate from project selection. A profile
   should select an organization, while `riichi project use` selects a project
   within that organization.
3. Start the guided loop as a prompt flow. A full-screen terminal UI is a
   later option only if the prompt flow proves too limiting.

One implementation question remains:

4. Which server endpoint should be the canonical issue-key resolver when an
   issue is attached to multiple projects but owned by one team?

### Resolver recommendation

Use the existing human issue read surface with an exact `display_key` filter,
scoped by the selected organization and optional project. The server should
return zero, one, or an explicit ambiguity result after authorization checks;
the CLI should never resolve keys by downloading an arbitrary issue list and
guessing locally.

This matches the current data model: display keys already include the team key
and sequence number, while project attachment is many-to-many. The owning team
is therefore the natural identity for the key, and project selection should be
used only to narrow the result when the user has selected a project. If the
current list endpoint cannot express those filters cleanly, add a small
authenticated resolver endpoint rather than coupling the CLI to SQL-shaped
responses.
