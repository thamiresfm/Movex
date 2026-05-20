export type LogLevel = "info" | "success" | "error" | "warning" | "sec" | "warn";

const entries: { time: string; level: string; msg: string }[] = [];
const MAX = 100;

export function addLog(msg: string, level: string = "info") {
  const time = new Date().toLocaleTimeString("pt-BR", { hour:"2-digit", minute:"2-digit", second:"2-digit" });
  entries.push({ time, level, msg });
  if (entries.length > MAX) entries.shift();
  renderLogs();
}

export function clearLogs() {
  entries.length = 0;
  renderLogs();
}

function renderLogs() {
  const el = document.getElementById("logBody");
  if (!el) return;
  const fragment = document.createDocumentFragment();
  for (const e of entries) {
    const line = document.createElement("div");
    line.className = "log-line" + (e.level === "warn" ? " warn-line" : "");

    const time = document.createElement("span");
    time.className = "log-time";
    time.textContent = e.time;

    const tag = document.createElement("span");
    tag.className = `log-tag ${e.level}`;
    tag.textContent = e.level.toUpperCase();

    const msg = document.createElement("span");
    msg.className = "log-msg";
    msg.textContent = e.msg;

    line.appendChild(time);
    line.appendChild(tag);
    line.appendChild(msg);
    fragment.appendChild(line);
  }
  el.replaceChildren(fragment);
  el.scrollTop = el.scrollHeight;
}
