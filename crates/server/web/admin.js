// Typeset setup and admin page. No build step: plain DOM, one file.
// All text goes in through textContent, never innerHTML.

const app = document.getElementById("app");

/** Creates an element. `props` are DOM properties; `on*` are listeners. */
function h(tag, props, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (key.startsWith("on")) el.addEventListener(key.slice(2), value);
    else if (key === "class") el.className = value;
    else if (value !== undefined && value !== null) el[key] = value;
  }
  for (const child of children.flat()) {
    if (child === null || child === undefined || child === false) continue;
    el.append(child instanceof Node ? child : String(child));
  }
  return el;
}

async function api(method, path, body) {
  const res = await fetch(`/admin/api/${path}`, {
    method,
    headers: body === undefined ? {} : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (res.status === 204) return null;
  const text = await res.text();
  let data = null;
  try {
    data = JSON.parse(text);
  } catch {
    // Not JSON: fall through to the plain text.
  }
  if (!res.ok) throw new Error(data?.error ?? (text || `Request failed (${res.status})`));
  return data;
}

/** A line of text that reports the result of an action. */
function statusLine() {
  const el = h("span", { class: "status", role: "status" });
  return {
    el,
    ok: (text) => ((el.className = "status ok"), (el.textContent = text)),
    bad: (text) => ((el.className = "status bad"), (el.textContent = text)),
    busy: (text) => ((el.className = "status muted"), (el.textContent = text)),
    clear: () => (el.textContent = ""),
  };
}

/** Runs `work` with the button disabled and failures shown in `status`. */
async function run(button, status, busyText, work) {
  button.disabled = true;
  status.busy(busyText);
  try {
    await work();
  } catch (e) {
    status.bad(e.message);
  } finally {
    button.disabled = false;
  }
}

function page(title, ...children) {
  return [h("header", { class: "top" }, h("h1", {}, title)), ...children];
}

function show(wide, ...nodes) {
  app.className = wide ? "wide" : "";
  app.replaceChildren(...nodes.flat());
}

// ---------------------------------------------------------------------------
// Screens before sign-in

function lockedScreen() {
  const token = h("input", { type: "password", id: "token", autocomplete: "off" });
  const status = statusLine();
  const button = h("button", { class: "primary", type: "submit" }, "Unlock setup");
  const form = h(
    "form",
    {
      class: "card",
      onsubmit: (e) => {
        e.preventDefault();
        run(button, status, "Checking…", async () => {
          await api("POST", "unlock", { token: token.value });
          await start();
        });
      },
    },
    h("h2", {}, "Enter the setup token"),
    h(
      "p",
      { class: "muted" },
      "This server is not set up yet. To prove you run it, paste the setup token. ",
      "The server prints a new token to its log each time it starts:",
    ),
    h("p", {}, h("code", {}, "journalctl -u typeset | grep setup")),
    h("label", { htmlFor: "token" }, "Setup token"),
    token,
    h("div", { class: "row" }, button, status.el),
  );
  show(false, page("Set up Typeset", form));
  token.focus();
}

function signInScreen(state) {
  const forbidden = state.phase === "forbidden";
  const who = state.user?.email ?? state.user?.name ?? "This account";
  const status = statusLine();
  const signOut = h(
    "button",
    {
      onclick: () =>
        run(signOut, status, "Signing out…", async () => {
          await api("POST", "logout");
          await start();
        }),
    },
    "Sign out",
  );
  show(
    false,
    page(
      "Typeset server",
      h(
        "div",
        { class: "card" },
        forbidden
          ? [
              h("h2", {}, "Not an admin"),
              h(
                "p",
                { class: "muted" },
                `${who} can use Typeset, but is not an admin of this server. `,
                "An admin can change that on this page.",
              ),
              h("div", { class: "row" }, signOut, status.el),
            ]
          : [
              h("h2", {}, "Admin sign-in"),
              h(
                "p",
                { class: "muted" },
                "This page manages the server. To use Typeset itself, open the desktop app ",
                "and enter this server's address.",
              ),
              h(
                "div",
                { class: "row" },
                h("a", { class: "button primary", href: "/admin/login" }, "Sign in"),
              ),
            ],
      ),
    ),
  );
}

// ---------------------------------------------------------------------------
// Settings form (used by setup and by the admin page)

function settingsForm(settings, { numbered, saveLabel, onSaved }) {
  const f = settings.fields;
  const lockedHint = (field) =>
    field.locked
      ? h(
          "p",
          { class: "hint" },
          "Set by ",
          h("code", {}, field.env),
          " in the server's environment. Remove it there to edit it here.",
        )
      : null;
  const heading = (n, text) =>
    h("h2", {}, numbered ? h("span", { class: "step" }, n) : null, text);

  // 1. Public URL
  const publicUrl = h("input", {
    type: "url",
    id: "publicUrl",
    value: settings.publicUrlKnown ? f.publicUrl.value : location.origin,
    disabled: f.publicUrl.locked,
    required: true,
  });
  const callback = h("code", {});
  const updateCallback = () => {
    callback.textContent = `${publicUrl.value.trim().replace(/\/+$/, "")}/auth/callback`;
  };
  publicUrl.addEventListener("input", updateCallback);
  updateCallback();
  const copied = statusLine();
  const copy = h(
    "button",
    {
      type: "button",
      class: "compact",
      onclick: async () => {
        try {
          await navigator.clipboard.writeText(callback.textContent);
          copied.ok("Copied");
        } catch {
          copied.bad("Copy it by hand");
        }
      },
    },
    "Copy",
  );

  // 2. Identity provider
  const issuer = h("input", {
    type: "url",
    id: "issuer",
    value: f.oidcIssuer.value,
    disabled: f.oidcIssuer.locked,
    placeholder: "https://auth.example.com",
    required: true,
  });
  const clientId = h("input", {
    type: "text",
    id: "clientId",
    value: f.oidcClientId.value,
    disabled: f.oidcClientId.locked,
    autocomplete: "off",
    required: true,
  });
  const secretSet = f.oidcClientSecret.value;
  const clientSecret = h("input", {
    type: "password",
    id: "clientSecret",
    disabled: f.oidcClientSecret.locked,
    autocomplete: "off",
    placeholder: secretSet ? "Saved. Leave empty to keep it." : "Leave empty for a public client",
  });
  const clearSecret = h("input", { type: "checkbox", id: "clearSecret" });
  const issuerStatus = statusLine();
  const testIssuer = h(
    "button",
    {
      type: "button",
      onclick: () =>
        run(testIssuer, issuerStatus, "Contacting the provider…", async () => {
          const r = await api("POST", "test/oidc", { issuer: issuer.value });
          issuerStatus.ok(`Found. Sign-in goes to ${new URL(r.authorizationEndpoint).host}.`);
        }),
    },
    "Test issuer",
  );

  // 3. Google Fonts
  const keySet = f.googleFontsApiKey.value;
  const apiKey = h("input", {
    type: "password",
    id: "apiKey",
    disabled: f.googleFontsApiKey.locked,
    autocomplete: "off",
    placeholder: keySet ? "Saved. Leave empty to keep it." : "",
    required: !keySet,
  });
  const keyStatus = statusLine();
  const testKey = h(
    "button",
    {
      type: "button",
      onclick: () =>
        run(testKey, keyStatus, "Asking Google Fonts…", async () => {
          const r = await api("POST", "test/fonts", { apiKey: apiKey.value });
          keyStatus.ok(`Works. Google lists ${r.families.toLocaleString()} font families.`);
        }),
    },
    "Test key",
  );

  // 4. Who may sign in
  const emails = h("input", {
    type: "text",
    id: "emails",
    value: f.allowedEmails.value,
    disabled: f.allowedEmails.locked,
    placeholder: "you@example.com, friend@example.com",
  });

  const saveStatus = statusLine();
  const save = h("button", { class: "primary", type: "submit" }, saveLabel);

  return h(
    "form",
    {
      onsubmit: (e) => {
        e.preventDefault();
        run(save, saveStatus, "Checking and saving…", async () => {
          await api("PUT", "settings", {
            publicUrl: publicUrl.value,
            oidcIssuer: issuer.value,
            oidcClientId: clientId.value,
            oidcClientSecret: clearSecret.checked ? "" : clientSecret.value || null,
            allowedEmails: emails.value,
            googleFontsApiKey: apiKey.value || null,
          });
          saveStatus.ok("Saved.");
          await onSaved();
        });
      },
    },
    h(
      "section",
      { class: "card" },
      heading(1, "Server address"),
      h("p", { class: "muted" }, "The address people and your identity provider use to reach this server."),
      h("label", { htmlFor: "publicUrl" }, "Public URL"),
      publicUrl,
      lockedHint(f.publicUrl),
    ),
    h(
      "section",
      { class: "card" },
      heading(2, "Sign-in"),
      h(
        "p",
        { class: "muted" },
        "Typeset signs people in through your own OIDC provider (Pocket ID, Authentik, ",
        "Keycloak, Authelia…). Create a client there with this callback URL and the scopes ",
        h("code", {}, "openid email profile"),
        ", then enter its details.",
      ),
      h("label", {}, "Callback URL to register"),
      h("div", { class: "copy" }, callback, copy, copied.el),
      h("label", { htmlFor: "issuer" }, "Issuer URL"),
      issuer,
      lockedHint(f.oidcIssuer),
      h("label", { htmlFor: "clientId" }, "Client ID"),
      clientId,
      lockedHint(f.oidcClientId),
      h("label", { htmlFor: "clientSecret" }, "Client secret"),
      clientSecret,
      lockedHint(f.oidcClientSecret),
      secretSet && !f.oidcClientSecret.locked
        ? h("label", { class: "check", htmlFor: "clearSecret" }, clearSecret, "Remove the saved secret")
        : null,
      h("div", { class: "row" }, testIssuer, issuerStatus.el),
      h(
        "p",
        { class: "hint" },
        "Changing the issuer later gives everyone a new, empty account.",
      ),
    ),
    h(
      "section",
      { class: "card" },
      heading(3, "Google Fonts"),
      h(
        "p",
        { class: "muted" },
        "The server reads the font catalog with a free Google Fonts Developer API key. ",
        h(
          "a",
          {
            href: "https://developers.google.com/fonts/docs/developer_api#APIKey",
            target: "_blank",
            rel: "noreferrer",
          },
          "Get a key",
        ),
        ".",
      ),
      h("label", { htmlFor: "apiKey" }, "API key"),
      apiKey,
      lockedHint(f.googleFontsApiKey),
      h("div", { class: "row" }, testKey, keyStatus.el),
    ),
    h(
      "section",
      { class: "card" },
      heading(4, "Who may sign in"),
      h("label", { htmlFor: "emails" }, "Allowed emails (optional)"),
      emails,
      lockedHint(f.allowedEmails),
      h(
        "p",
        { class: "hint" },
        "Leave empty to let in everyone your identity provider lets in.",
      ),
    ),
    h("div", { class: "row" }, save, saveStatus.el),
  );
}

async function setupScreen() {
  const settings = await api("GET", "settings");
  show(
    false,
    page(
      "Set up Typeset",
      h(
        "p",
        { class: "muted" },
        "Four short steps. The sign-in and Google Fonts details are tested before anything is saved.",
      ),
      settingsForm(settings, { numbered: true, saveLabel: "Save and continue", onSaved: start }),
    ),
  );
}

async function claimScreen() {
  const settings = await api("GET", "settings");
  show(
    false,
    page(
      "Set up Typeset",
      h(
        "div",
        { class: "card" },
        h("h2", {}, "Last step: become the admin"),
        h(
          "p",
          { class: "muted" },
          "The server is configured. Sign in through your identity provider now, and that ",
          "account becomes this server's admin. This also proves that sign-in works.",
        ),
        h(
          "div",
          { class: "row" },
          h("a", { class: "button primary", href: "/admin/login" }, "Sign in and become admin"),
        ),
      ),
      h(
        "details",
        {},
        h("summary", { class: "muted" }, "Change the settings"),
        h("p", {}),
        settingsForm(settings, { numbered: false, saveLabel: "Save", onSaved: start }),
      ),
    ),
  );
}

// ---------------------------------------------------------------------------
// Admin page

function bytes(n) {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = n;
  let unit = -1;
  do {
    value /= 1024;
    unit += 1;
  } while (value >= 1024 && unit < units.length - 1);
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

function ago(now, then) {
  const s = Math.max(0, now - then);
  if (s < 90) return "just now";
  if (s < 3600 * 1.5) return `${Math.round(s / 60)} min ago`;
  if (s < 86400 * 1.5) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}

const day = (t) => new Date(t * 1000).toLocaleDateString();

function table(headers, rows, empty) {
  if (rows.length === 0) return h("p", { class: "muted" }, empty);
  return h(
    "div",
    { class: "scroll" },
    h(
      "table",
      {},
      h("thead", {}, h("tr", {}, headers.map((t) => h("th", { scope: "col" }, t)))),
      h("tbody", {}, rows),
    ),
  );
}

async function adminScreen(state) {
  const [data, settings] = await Promise.all([api("GET", "overview"), api("GET", "settings")]);
  const status = statusLine();
  const userName = (u) => u.email ?? u.name ?? "(no email)";
  const byId = new Map(data.users.map((u) => [u.id, u]));

  /** A button that calls the API, then redraws the page. */
  const action = (label, opts, work) => {
    const button = h(
      "button",
      {
        class: `compact ${opts.danger ? "danger" : ""}`,
        onclick: () => {
          if (opts.confirm && !confirm(opts.confirm)) return;
          run(button, status, "Working…", async () => {
            await work();
            await start();
          });
        },
      },
      label,
    );
    return button;
  };

  const refreshStatus = statusLine();
  const refresh = h(
    "button",
    {
      class: "compact",
      onclick: () =>
        run(refresh, refreshStatus, "Fetching…", async () => {
          await api("POST", "catalog/refresh");
          await start();
        }),
    },
    "Refresh now",
  );

  const userRows = data.users.map((u) =>
    h(
      "tr",
      {},
      h(
        "td",
        {},
        userName(u),
        u.isAdmin ? h("span", { class: "tag" }, "admin") : null,
        u.id === data.me ? h("span", { class: "tag" }, "you") : null,
      ),
      h("td", {}, u.name ?? ""),
      h("td", {}, u.libraryCount.toLocaleString()),
      h("td", {}, u.uploadCount ? `${u.uploadCount} (${bytes(u.uploadBytes)})` : "0"),
      h("td", {}, day(u.createdAt)),
      h(
        "td",
        { class: "actions" },
        u.id === data.me
          ? null
          : [
              u.isAdmin
                ? action("Remove admin", {}, () => api("PUT", `users/${u.id}`, { isAdmin: false }))
                : action("Make admin", {}, () => api("PUT", `users/${u.id}`, { isAdmin: true })),
              action(
                "Delete",
                {
                  danger: true,
                  confirm: `Delete ${userName(u)}? This removes their library, collections and uploaded fonts from the server. It cannot be undone.`,
                },
                () => api("DELETE", `users/${u.id}`),
              ),
            ],
      ),
    ),
  );

  const sessionRows = data.sessions.map((s) =>
    h(
      "tr",
      {},
      h("td", {}, s.deviceName ?? "Unnamed device"),
      h("td", {}, userName(byId.get(s.userId) ?? {})),
      h("td", {}, ago(data.now, s.lastUsedAt)),
      h("td", {}, day(s.createdAt)),
      h(
        "td",
        { class: "actions" },
        action("Sign out", { confirm: "Sign this device out?" }, () =>
          api("DELETE", `sessions/${s.id}`),
        ),
      ),
    ),
  );

  const signOut = h(
    "button",
    {
      onclick: () =>
        run(signOut, status, "Signing out…", async () => {
          await api("POST", "logout");
          await start();
        }),
    },
    "Sign out",
  );

  show(
    true,
    h(
      "header",
      { class: "top" },
      h("h1", {}, "Typeset server"),
      h(
        "div",
        { class: "row" },
        h("span", { class: "muted small" }, state.user?.email ?? state.user?.name ?? ""),
        signOut,
      ),
    ),
    h("p", {}, status.el),
    h(
      "div",
      { class: "stats" },
      h(
        "div",
        { class: "card" },
        h("div", { class: "muted small" }, "People"),
        h("div", { class: "stat" }, data.users.length),
        h("div", { class: "muted small" }, `${data.sessions.length} signed-in devices`),
      ),
      h(
        "div",
        { class: "card" },
        h("div", { class: "muted small" }, "Uploaded fonts"),
        h("div", { class: "stat" }, bytes(data.uploads.bytes)),
        h("div", { class: "muted small" }, `${data.uploads.files} files`),
      ),
      h(
        "div",
        { class: "card" },
        h("div", { class: "muted small" }, "Google Fonts catalog"),
        h("div", { class: "stat" }, data.catalog ? data.catalog.families.toLocaleString() : "None"),
        h(
          "div",
          { class: "muted small" },
          data.catalog ? `families, fetched ${ago(data.now, data.catalog.fetchedAt)}` : "not fetched yet",
        ),
        h("div", { class: "row" }, refresh, refreshStatus.el),
      ),
    ),
    h(
      "section",
      { class: "card" },
      h("h2", {}, "People"),
      table(["Email", "Name", "Library", "Uploads", "Joined", ""], userRows, "Nobody has signed in yet."),
    ),
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Devices"),
      table(["Device", "Person", "Last used", "Signed in", ""], sessionRows, "No devices are signed in."),
    ),
    h("h2", {}, "Settings"),
    h("p", { class: "muted" }, "Changes apply at once. No restart is needed."),
    settingsForm(settings, { numbered: false, saveLabel: "Save settings", onSaved: start }),
    h("p", { class: "muted small" }, `Typeset server ${data.version}`),
  );
}

// ---------------------------------------------------------------------------

async function start() {
  try {
    const state = await api("GET", "state");
    if (state.phase === "locked") lockedScreen();
    else if (state.phase === "setup") await setupScreen();
    else if (state.phase === "claim") await claimScreen();
    else if (state.phase === "ready") await adminScreen(state);
    else signInScreen(state);
  } catch (e) {
    show(false, page("Typeset server", h("p", { class: "status bad" }, e.message)));
  }
}

start();
