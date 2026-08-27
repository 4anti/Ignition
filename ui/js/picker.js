const T = () => window.__TAURI__;
const invoke = (cmd, args) => T().core.invoke(cmd, args);

const listEl = document.getElementById("list");
const emptyEl = document.getElementById("empty");
const statusEl = document.getElementById("status-line");
const countEl = document.getElementById("unit-count");
const autoEl = document.getElementById("autostart");
const delayEl = document.getElementById("login-delay");
const staggerEl = document.getElementById("stagger");
const urlForm = document.getElementById("url-form");
const urlInput = document.getElementById("url-input");

let state = {
  config: { items: [], loginDelayMs: 0, staggerMs: 350 },
  autostart: false,
  icons: {},
};

function pad(n) {
  return String(n).padStart(2, "0");
}

function setStatus(text, isError) {
  statusEl.textContent = text;
  statusEl.classList.toggle("is-error", !!isError);
  document.querySelector(".dot")?.classList.toggle("is-error", !!isError);
}

function apply(next) {
  state = next;
  autoEl.checked = !!next.autostart;
  delayEl.value = String(Math.round((next.config.loginDelayMs || 0) / 1000));
  staggerEl.value = String(((next.config.staggerMs || 0) / 1000).toFixed(2).replace(/\.00$/, ""));
  render();
}

function delayValue(ms) {
  const sec = (Number(ms) || 0) / 1000;
  return String(sec).replace(/\.0$/, "");
}

function render() {
  const items = state.config.items || [];
  const icons = state.icons || {};
  countEl.textContent = items.length === 1 ? "1 item" : `${items.length} items`;
  emptyEl.hidden = items.length > 0;
  listEl.replaceChildren();

  items.forEach((item, i) => {
    const row = document.createElement("li");
    row.className = `row${item.enabled ? " enabled" : ""}`;
    row.style.setProperty("--i", String(i));
    row.dataset.id = item.id;

    const icon = icons[item.id]
      ? `<img class="file-icon" src="${icons[item.id]}" alt="" />`
      : `<span class="file-icon missing" aria-hidden="true"></span>`;

    const args =
      item.kind === "app"
        ? `<input class="args" data-act="args" placeholder="args" value="${escapeAttr(
            item.args || ""
          )}" />`
        : `<span></span>`;

    row.innerHTML = `
      <span class="idx">${pad(i + 1)}</span>
      <button type="button" class="arm" data-act="toggle" aria-label="${
        item.enabled ? "Disable" : "Enable"
      }"></button>
      <div class="who">
        ${icon}
        <div>
          <strong>${escapeHtml(item.name)}</strong>
          <small title="${escapeAttr(item.target)}">${escapeHtml(item.target)}</small>
        </div>
      </div>
      <span class="chip">${escapeHtml(item.kind)}</span>
      <label class="wait">Wait <input class="delay" data-act="delay" type="number" min="0" max="120" step="0.5" value="${delayValue(
        item.delayMs
      )}" /> s</label>
      ${args}
      <button type="button" class="icon-btn" data-act="play">Open</button>
      <button type="button" class="icon-btn danger" data-act="remove">Remove</button>
    `;
    listEl.appendChild(row);
  });
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(s) {
  return escapeHtml(s);
}

async function mutate(cmd, args) {
  try {
    const next = await invoke(cmd, args);
    apply(next);
    return next;
  } catch (err) {
    setStatus("Couldn't save", true);
    throw err;
  }
}

function currentWindow() {
  try {
    return T()?.window?.getCurrentWindow?.() ?? null;
  } catch {
    return null;
  }
}

async function boot() {
  const win = currentWindow();
  if (win) {
    document.getElementById("min-btn").onclick = () => win.minimize();
    document.getElementById("max-btn").onclick = () => win.toggleMaximize();
    document.getElementById("close-btn").onclick = () => win.close();
  }

  const canInvoke = () => !!T()?.core?.invoke;
  const addFile = () => {
    if (!canInvoke()) return;
    mutate("pick_and_add", { mode: "file" }).then(() => setStatus("Added"));
  };
  const addFolder = () => {
    if (!canInvoke()) return;
    mutate("pick_and_add", { mode: "folder" }).then(() => setStatus("Added"));
  };
  const showUrl = () => {
    urlForm.hidden = false;
    urlInput.focus();
  };

  document.getElementById("add-file").onclick = addFile;
  document.getElementById("add-folder").onclick = addFolder;
  document.getElementById("add-url").onclick = showUrl;
  document.querySelectorAll("[data-add]").forEach((btn) => {
    btn.onclick = () => {
      const kind = btn.getAttribute("data-add");
      if (kind === "file") addFile();
      else if (kind === "folder") addFolder();
      else showUrl();
    };
  });
  document.getElementById("url-cancel").onclick = () => {
    urlForm.hidden = true;
    urlInput.value = "";
  };
  urlForm.onsubmit = async (e) => {
    e.preventDefault();
    await mutate("add_url", { url: urlInput.value });
    urlInput.value = "";
    urlForm.hidden = true;
    setStatus("Added");
  };

  autoEl.onchange = () =>
    mutate("set_autostart", { enabled: autoEl.checked }).then(() =>
      setStatus(autoEl.checked ? "Login on" : "Login off")
    );

  const saveDelays = () => {
    const loginDelayMs = Math.max(0, Number(delayEl.value) || 0) * 1000;
    const staggerMs = Math.max(0, Number(staggerEl.value) || 0) * 1000;
    mutate("set_delays", { loginDelayMs, staggerMs }).then(() => setStatus("Timing set"));
  };
  delayEl.onchange = saveDelays;
  staggerEl.onchange = saveDelays;

  document.getElementById("launch-now").onclick = async () => {
    setStatus("Launching");
    const report = await invoke("launch_enabled");
    if (report.failed) setStatus(`${report.launched} opened, ${report.failed} failed`, true);
    else setStatus(`${report.launched} opened`);
  };

  listEl.addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-act]");
    if (!btn || btn.tagName === "INPUT") return;
    const row = btn.closest(".row");
    const id = row.dataset.id;
    const act = btn.dataset.act;
    if (act === "toggle") {
      const item = state.config.items.find((i) => i.id === id);
      await mutate("set_enabled", { id, enabled: !item.enabled });
    } else if (act === "remove") {
      await mutate("remove_item", { id });
      setStatus("Removed");
    } else if (act === "play") {
      setStatus("Opening…");
      try {
        const result = await invoke("launch_one", { id });
        row.classList.toggle("fail", !result.ok);
        setStatus(
          result.ok ? `Opened ${result.name}` : result.error || "Couldn't open",
          !result.ok
        );
      } catch (err) {
        row.classList.add("fail");
        setStatus(String(err), true);
      }
    }
  });

  listEl.addEventListener("change", async (e) => {
    const input = e.target.closest("input[data-act]");
    if (!input) return;
    const id = input.closest(".row").dataset.id;
    if (input.dataset.act === "args") {
      await mutate("set_args", { id, args: input.value });
    } else if (input.dataset.act === "delay") {
      const delayMs = Math.round(
        Math.max(0, Math.min(120, Number(String(input.value).replace(",", ".")) || 0)) * 1000
      );
      await mutate("set_item_delay", { id, delayMs });
    }
  });

  if (!canInvoke()) {
    setStatus("Ready");
    return;
  }

  await T().event.listen("drag-state", (event) => {
    document.body.classList.toggle("dragging", !!event.payload);
  });

  await T().event.listen("paths-dropped", async (event) => {
    document.body.classList.remove("dragging");
    try {
      await mutate("add_targets", { paths: event.payload });
      setStatus("Added");
    } catch (err) {
      setStatus(String(err));
    }
  });

  apply(await invoke("get_state"));
  setStatus("Ready");
}

boot().catch((err) => {
  console.error(err);
  setStatus("Couldn't load");
});
