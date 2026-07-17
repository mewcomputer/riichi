import { createHmac, randomUUID } from "node:crypto";
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { once } from "node:events";
import { spawn, type ChildProcess } from "node:child_process";
import { promisify } from "node:util";

import { expect, test, type Page } from "@playwright/test";
import { PostgreSqlContainer, type StartedPostgreSqlContainer } from "@testcontainers/postgresql";
import {
  GenericContainer,
  Network,
  type StartedNetwork,
  type StartedTestContainer,
  Wait,
} from "testcontainers";
import { Pool } from "pg";

const repositoryRoot = new URL("../../../../", import.meta.url).pathname;
const webRoot = new URL("../../", import.meta.url).pathname;

let postgres: StartedPostgreSqlContainer;
let network: StartedNetwork;
let electric: StartedTestContainer;
let database: Pool;
let provider: Server;
let providerPort: number;
let api: ChildProcess;
let web: ChildProcess;
let webPort: number;
let webUrl: string;

function freePort() {
  return new Promise<number>((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("could not allocate a local port"));
        return;
      }
      server.close(() => resolve(address.port));
    });
  });
}

function base64Url(value: string) {
  return Buffer.from(value).toString("base64url");
}

function signedIdToken(issuer: string, nonce: string) {
  const header = base64Url(JSON.stringify({ alg: "HS256", typ: "JWT" }));
  const payload = base64Url(
    JSON.stringify({
      iss: issuer,
      aud: "riichi",
      sub: "queue-e2e-user",
      email: "queue@example.test",
      email_verified: true,
      preferred_username: "Queue E2E User",
      nonce,
      iat: Math.floor(Date.now() / 1000),
      exp: Math.floor(Date.now() / 1000) + 300,
    }),
  );
  const message = `${header}.${payload}`;
  const signature = createHmac("sha256", "secret").update(message).digest("base64url");
  return `${message}.${signature}`;
}

async function readBody(request: IncomingMessage) {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(Buffer.from(chunk));
  return Buffer.concat(chunks).toString("utf8");
}

async function startOidcProvider() {
  providerPort = await freePort();
  const issuer = `http://127.0.0.1:${providerPort}`;
  const codes = new Map<string, string>();

  provider = createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", issuer);
    if (url.pathname === "/.well-known/openid-configuration") {
      return sendJson(response, {
        issuer,
        authorization_endpoint: `${issuer}/authorize`,
        token_endpoint: `${issuer}/token`,
        jwks_uri: `${issuer}/jwks`,
        response_types_supported: ["code"],
        subject_types_supported: ["public"],
        id_token_signing_alg_values_supported: ["HS256"],
        scopes_supported: ["openid", "profile", "email"],
      });
    }
    if (url.pathname === "/jwks") {
      return sendJson(response, {
        keys: [{ kty: "oct", k: "c2VjcmV0", alg: "HS256", use: "sig" }],
      });
    }
    if (url.pathname === "/authorize") {
      const code = randomUUID();
      const nonce = url.searchParams.get("nonce");
      const redirectUri = url.searchParams.get("redirect_uri");
      const state = url.searchParams.get("state");
      if (!nonce || !redirectUri || !state) {
        response.writeHead(400).end("missing authorization parameters");
        return;
      }
      codes.set(code, nonce);
      const callback = new URL(redirectUri);
      callback.searchParams.set("code", code);
      callback.searchParams.set("state", state);
      response.writeHead(302, { location: callback.toString() }).end();
      return;
    }
    if (url.pathname === "/token" && request.method === "POST") {
      const form = new URLSearchParams(await readBody(request));
      const code = form.get("code");
      const nonce = code ? codes.get(code) : undefined;
      if (!nonce || form.get("code_verifier") === null) {
        return sendJson(response, { error: "invalid_grant" }, 400);
      }
      return sendJson(response, {
        access_token: "e2e-access-token",
        token_type: "Bearer",
        expires_in: 300,
        id_token: signedIdToken(issuer, nonce),
      });
    }
    response.writeHead(404).end();
  });
  await promisify(provider.listen.bind(provider))(providerPort, "127.0.0.1");
  return issuer;
}

function sendJson(response: ServerResponse, value: unknown, status = 200) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(value));
}

function startProcess(
  command: string,
  args: string[],
  env: NodeJS.ProcessEnv,
  cwd = repositoryRoot,
) {
  const child = spawn(command, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  child.stdout?.on("data", (chunk) => {
    output += chunk.toString();
  });
  child.stderr?.on("data", (chunk) => {
    output += chunk.toString();
  });
  child.on("exit", (code, signal) => {
    if (code !== 0 && signal !== "SIGTERM") {
      output += `\nprocess exited with code=${code} signal=${signal}`;
    }
  });
  return { child, getOutput: () => output };
}

async function waitForHttp(url: string, getOutput: () => string) {
  const deadline = Date.now() + 90_000;
  let lastError = "";
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok || response.status === 401 || response.status === 404) return;
      lastError = `${response.status} ${await response.text()}`;
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timed out waiting for ${url}: ${lastError}\n${getOutput()}`);
}

async function stopProcess(child: ChildProcess | undefined) {
  if (!child || child.exitCode !== null) return;
  child.kill("SIGTERM");
  await Promise.race([once(child, "exit"), new Promise((resolve) => setTimeout(resolve, 5_000))]);
  if (child.exitCode === null) child.kill("SIGKILL");
}

async function runProcess(command: string, args: string[], env: NodeJS.ProcessEnv) {
  const process = startProcess(command, args, env);
  const [code, signal] = await once(process.child, "exit") as [number | null, NodeJS.Signals | null];
  if (code !== 0) {
    throw new Error(`${command} exited with code=${code} signal=${signal}\n${process.getOutput()}`);
  }
}

async function seedDatabase(connectionString: string, issuer: string) {
  database = new Pool({ connectionString });
  const projectId = "11111111-1111-4111-8111-111111111111";
  const issueId = "22222222-2222-4222-8222-222222222222";
  const accountId = "33333333-3333-4333-8333-333333333333";
  await database.query(
    `INSERT INTO human_accounts (id, issuer, subject, email, display_name)
     VALUES ($1, $2, $3, $4, $5)`,
    [accountId, issuer, "queue-e2e-user", "queue@example.test", "Queue E2E User"],
  );
  await database.query(
    "INSERT INTO projects (id, name, organization_id) VALUES ($1, $2, '00000000-0000-0000-0000-000000000001')",
    [projectId, "E2E Project"],
  );
  await database.query(
    "INSERT INTO project_memberships (project_id, account_id, role) VALUES ($1, $2, 'owner')",
    [projectId, accountId],
  );
  await database.query(
    "INSERT INTO organization_memberships (organization_id, account_id, role) VALUES ('00000000-0000-0000-0000-000000000001', $1, 'owner')",
    [accountId],
  );
  await database.query(
    "INSERT INTO team_memberships (team_id, account_id, role) VALUES ('00000000-0000-0000-0000-000000000002', $1, 'owner')",
    [accountId],
  );
  await database.query(
    "INSERT INTO project_teams (project_id, team_id, role) VALUES ($1, '00000000-0000-0000-0000-000000000002', 'admin')",
    [projectId],
  );
  await database.query(
    `INSERT INTO issues (id, project_id, team_id, display_key, title, body, status, agent_eligible, spec_complete)
     VALUES ($1, $2, '00000000-0000-0000-0000-000000000002', $3, $4, $5, 'todo', true, true)`,
    [issueId, projectId, "RII-E2E-1", "Verify the real browser queue", "Seeded through PostgreSQL."],
  );
  await database.query("INSERT INTO issue_dispatch (issue_id, rank) VALUES ($1, 0)", [issueId]);
  const descriptionDocumentId = "44444444-4444-4444-8444-444444444444";
  const descriptionContent = {
    type: "doc",
    content: [{ type: "paragraph", content: [{ type: "text", text: "Seeded description." }] }],
  };
  await database.query(
    `INSERT INTO documents
       (id, organization_id, kind, title, owner_team_id, provisioning_state, created_by)
     VALUES ($1, '00000000-0000-0000-0000-000000000001', 'issue_description', $2,
       '00000000-0000-0000-0000-000000000002', 'ready', $3)`,
    [descriptionDocumentId, "RII-E2E-1 description", accountId],
  );
  await database.query(
    `INSERT INTO document_versions
       (document_id, revision, content, plain_text, sanitized_html, schema_version, created_by)
     VALUES ($1, 1, $2, 'Seeded description.', '<p>Seeded description.</p>', 1, $3)`,
    [descriptionDocumentId, descriptionContent, accountId],
  );
  await database.query(
    `INSERT INTO document_projections
       (document_id, content_revision, plain_text, sanitized_html, schema_version)
     VALUES ($1, 1, 'Seeded description.', '<p>Seeded description.</p>', 1)`,
    [descriptionDocumentId],
  );
  await database.query(
    `INSERT INTO document_bindings (document_id, resource_kind, resource_id, role)
     VALUES ($1, 'issue', $2, 'description')`,
    [descriptionDocumentId, issueId],
  );
  return projectId;
}

async function signIn(page: Page) {
  await page.goto(webUrl);
  await expect(page.getByRole("link", { name: "Continue with Pocket ID" })).toBeVisible();
  await page.getByRole("link", { name: "Continue with Pocket ID" }).click();
  await expect(page.getByText("RII-E2E-1")).toBeVisible();
}

test.beforeAll(async () => {
  const issuer = await startOidcProvider();
  network = await new Network().start();
  postgres = await new PostgreSqlContainer("postgres:16-alpine")
    .withDatabase("riichi")
    .withUsername("postgres")
    .withPassword("postgres")
    .withCommand(["postgres", "-c", "wal_level=logical"])
    .withNetwork(network)
    .withNetworkAliases("riichi-postgres")
    .start();
  const commonEnvironment = {
    RIICHI_DATABASE_URL: postgres.getConnectionUri(),
    RIICHI_DATABASE_MAX_CONNECTIONS: "5",
  };
  await runProcess("cargo", ["run", "--quiet", "--bin", "riichi-migrate"], commonEnvironment);
  electric = await new GenericContainer("electricsql/electric:latest")
    .withNetwork(network)
    .withEnvironment({
      DATABASE_URL: "postgres://postgres:postgres@riichi-postgres:5432/riichi",
      ELECTRIC_PORT: "5133",
      ELECTRIC_SECRET: "e2e-electric-secret",
      ELECTRIC_REPLICATION_STREAM_ID: "riichi_browser_e2e",
      ELECTRIC_STORAGE_DIR: "/var/lib/electric",
    })
    .withExposedPorts(5133)
    .withWaitStrategy(Wait.forListeningPorts())
    .start();
  const electricUrl = `http://${electric.getHost()}:${electric.getMappedPort(5133)}`;
  await waitForHttp(`${electricUrl}/v1/health`, () => "Electric container did not become ready");
  const apiPort = await freePort();
  webPort = await freePort();
  webUrl = `http://127.0.0.1:${webPort}`;
  const apiProcess = startProcess("cargo", ["run", "--quiet", "-p", "riichi-api", "--bin", "riichi-api"], {
    RIICHI_API_ADDR: `127.0.0.1:${apiPort}`,
    RIICHI_DATABASE_URL: postgres.getConnectionUri(),
    RIICHI_DATABASE_MAX_CONNECTIONS: "5",
    RIICHI_OIDC_ISSUER_URL: issuer,
    RIICHI_OIDC_CLIENT_ID: "riichi",
    RIICHI_OIDC_CLIENT_SECRET: "secret",
    RIICHI_OIDC_REDIRECT_URL: `${webUrl}/auth/callback`,
    RIICHI_AUTH_COOKIE_SECURE: "false",
    RIICHI_ELECTRIC_URL: electricUrl,
    RIICHI_ELECTRIC_SOURCE_SECRET: "e2e-electric-secret",
    RUST_LOG: "warn",
  });
  api = apiProcess.child;
  await waitForHttp(`http://127.0.0.1:${apiPort}/health`, apiProcess.getOutput);
  const projectId = await seedDatabase(postgres.getConnectionUri(), issuer);
  const webProcess = startProcess(
    "pnpm",
    ["exec", "vite", "--host", "127.0.0.1", "--port", String(webPort)],
    {
      VITE_RIICHI_PROJECT_ID: projectId,
      VITE_ELECTRIC_SYNC_ENABLED: "true",
      RIICHI_WEB_PROXY_TARGET: `http://127.0.0.1:${apiPort}`,
    },
    webRoot,
  );
  web = webProcess.child;
  await waitForHttp(webUrl, webProcess.getOutput);
});

test.afterAll(async () => {
  await stopProcess(web);
  await stopProcess(api);
  await database?.end();
  await electric?.stop();
  await postgres?.stop();
  await network?.stop();
  if (provider?.listening) await new Promise<void>((resolve) => provider.close(() => resolve()));
});

test("signs in through OIDC and renders the server queue", async ({ page }) => {
  await signIn(page);
  await expect(page.getByText("Queue E2E User")).toBeVisible();
  await expect(page.getByRole("tab", { name: /Active/ })).toBeVisible();
  await expect(page.getByText("synced from server")).toBeVisible();
});

test("keeps the queue usable after reload and supports filtering", async ({ page }) => {
  await signIn(page);
  await page.getByRole("textbox", { name: "Search issues" }).fill("does-not-exist");
  await expect(page.getByText("No issues found")).toBeVisible();
  await page.getByRole("textbox", { name: "Search issues" }).fill("RII-E2E-1");
  await expect(page.getByText("Verify the real browser queue")).toBeVisible();
  await page.reload();
  await expect(page.getByText("Verify the real browser queue")).toBeVisible();
});

test("opens organization documentation and creates a standalone page", async ({ page }) => {
  await signIn(page);
  await page.getByRole("button", { name: "Documentation", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Documentation", exact: true })).toBeVisible();
  await expect(page.getByText("No organization pages yet.", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "New page", exact: true }).click();
  await expect(page).toHaveURL(/\/riichi\/documents\/[0-9a-f-]+$/);
  await expect(page.getByLabel("Document title")).toHaveValue("Untitled document");
  await expect(page.locator('[contenteditable="true"][aria-label="Document content"]')).toBeVisible();
});

test("creates an issue and completes the approval and comment workflow", async ({ page }) => {
  await signIn(page);
  await page.getByRole("button", { name: "New issue" }).click();
  await page.getByLabel("Issue title").fill("Created from the browser");
  await page.getByLabel("Issue description").fill("Created through the real human API path.");
  await page.getByRole("button", { name: "Create issue" }).click();

  await expect(page.getByLabel("Issue title")).toHaveText("Created from the browser");
  await page.getByText("Approval request", { exact: true }).click();
  await page.getByLabel("Proposed rank").fill("3");
  await page.getByRole("button", { name: "Request approval" }).click();
  await expect(page.getByText("Request pending")).toBeVisible();
  await page.getByRole("button", { name: "Approve" }).click();
  await expect(page.getByText("Request approved")).toBeVisible();
  await page.getByLabel("Comment").fill("A rich comment from the browser.");
  await page.getByRole("button", { name: "Comment" }).click();
  await expect(page.getByRole("article").getByText("A rich comment from the browser.")).toBeVisible();
});

test("updates issue status and importance through the replicated metadata boundary", async ({ page }) => {
  await signIn(page);
  await page.getByText("Verify the real browser queue").click();
  await expect(page.getByRole("button", { name: "Todo", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Todo", exact: true }).click();
  await page.getByRole("menuitemradio", { name: "In progress", exact: true }).click();
  await expect(page.getByRole("button", { name: "In progress", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "No priority", exact: true }).click();
  await page.getByRole("menuitemradio", { name: "High", exact: true }).click();
  await expect(page.getByRole("button", { name: "High", exact: true })).toBeVisible();
});

test("pastes structured clipboard content into the Loro document editor", async ({ page }) => {
  await signIn(page);
  await page.getByText("Verify the real browser queue").click();
  const documentContent = page.locator('[contenteditable="true"][aria-label="Document content"]');
  await expect(documentContent).toBeVisible();
  await page.evaluate(async () => {
    const target = document.querySelector('[contenteditable="true"][aria-label="Document content"]');
    if (!(target instanceof HTMLElement)) throw new Error("document editor is unavailable");
    const clipboardData = new DataTransfer();
    clipboardData.setData("text/html", "<ul><li><p>Pasted checklist item</p></li></ul>");
    clipboardData.setData("text/plain", "Pasted checklist item");
    target.dispatchEvent(new ClipboardEvent("paste", {
      bubbles: true,
      cancelable: true,
      clipboardData,
    }));
  });
  // Chromium may expose only the plain-text clipboard flavor to an untrusted
  // synthetic paste event, so verify the user-visible fallback here. The
  // Loro/Tiptap block-shape path is covered by the editor unit test.
  await expect(documentContent).toContainText("Pasted checklist item");
});

test("loads the operator roster and creates an agent role through the API", async ({ page }) => {
  await signIn(page);
  await page.getByRole("button", { name: "Agents" }).click();
  await expect(page.getByRole("heading", { name: "Agent roster" })).toBeVisible();
  await page.getByLabel("New role name").fill("browser-review");
  await page.getByRole("button", { name: "Create role" }).click();
  await expect(page.getByRole("heading", { name: "browser-review" })).toBeVisible();
});
