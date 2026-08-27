const T = () => window.__TAURI__;

const statusEl = document.getElementById("status");
const currentEl = document.getElementById("current");
const countEl = document.getElementById("count");
const barEl = document.getElementById("bar");

function pad(n) {
  return String(n).padStart(2, "0");
}

async function boot() {
  if (!T()?.event?.listen) return;

  await T().event.listen("boot-status", (e) => {
    const p = e.payload;
    countEl.textContent = pad(p.total || 0);
    if (p.total === 0) {
      statusEl.textContent = "Idle";
      currentEl.textContent = "Nothing armed";
      barEl.style.transform = "scaleX(1)";
      return;
    }
    if (p.phase === "wait" && p.loginDelayMs > 0) {
      statusEl.textContent = "Holding";
      currentEl.textContent = `Delay ${Math.round(p.loginDelayMs / 1000)}s`;
    } else {
      statusEl.textContent = "Igniting";
      currentEl.textContent = "Opening selected units";
    }
  });

  await T().event.listen("boot-progress", (e) => {
    const p = e.payload;
    const frac = p.total ? (p.index + 1) / p.total : 1;
    barEl.style.transform = `scaleX(${frac})`;
    statusEl.textContent = p.ok ? "Live" : "Miss";
    currentEl.textContent = p.error ? `${p.name} · ${p.error}` : p.name;
    countEl.textContent = `${pad(p.index + 1)}/${pad(p.total)}`;
  });

  await T().event.listen("boot-done", (e) => {
    const r = e.payload;
    document.body.classList.add("done");
    barEl.style.transform = "scaleX(1)";
    statusEl.textContent = r.failed ? "Partial" : "Done";
    currentEl.textContent = r.launched
      ? `${r.launched} opened${r.failed ? ` · ${r.failed} miss` : ""}`
      : "Empty pass";
  });

  await T().core.invoke("start_boot");
}

boot().catch((err) => {
  statusEl.textContent = "Fault";
  currentEl.textContent = String(err);
});
