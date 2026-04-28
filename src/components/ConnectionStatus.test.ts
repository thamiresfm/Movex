/**
 * Estado de sessão esperado pela UI (get_status / eventos IPC).
 */
import { describe, it, expect } from "vitest";
import { normalizeStatusPayload } from "./ConnectionStatus";

describe("normalizeStatusPayload — conexão / sessão", () => {
  it("considera sessão activa só com texto «Aguardando conexão» (in_session false do backend)", () => {
    const p = normalizeStatusPayload({
      connected: false,
      in_session: false,
      status_text: "Aguardando conexão...",
      active_screen: "Local",
      uptime_secs: 0,
    });
    expect(p.connected).toBe(false);
    expect(p.in_session).toBe(true);
    expect(p.status_text).toContain("Aguardando");
  });

  it("mantém texto e sessão quando in_session vem explicitamente true", () => {
    const p = normalizeStatusPayload({
      connected: false,
      in_session: true,
      status_text: "A ligar…",
      active_screen: "Local",
      uptime_secs: 0,
    });
    expect(p.in_session).toBe(true);
  });

  it("ligaado = sempre em sessão", () => {
    const p = normalizeStatusPayload({
      connected: true,
      in_session: false,
      status_text: "Conectado",
      active_screen: "Remote",
      uptime_secs: 10,
    });
    expect(p.connected).toBe(true);
    expect(p.in_session).toBe(true);
  });

  it("camelCase vindos do serde", () => {
    const p = normalizeStatusPayload({
      connected: true,
      inSession: false,
      statusText: "OK",
      activeScreen: "Remote",
      latencyMs: 12,
      uptimeSecs: 5,
    });
    expect(p.in_session).toBe(true);
    expect(p.status_text).toBe("OK");
    expect(p.latency_ms).toBe(12);
    expect(p.uptime_secs).toBe(5);
  });

  it("payload inválido → desligado por omissão", () => {
    const p = normalizeStatusPayload(null);
    expect(p.connected).toBe(false);
    expect(p.in_session).toBe(false);
  });
});
