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
  el.innerHTML = entries.map(e =>
    `<div class="log-line${e.level==='warn'?' warn-line':''}">
      <span class="log-time">${e.time}</span>
      <span class="log-tag ${e.level}">${e.level.toUpperCase()}</span>
      <span class="log-msg">${e.msg}</span>
    </div>`
  ).join('');
  el.scrollTop = el.scrollHeight;
}
