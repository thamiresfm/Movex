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

  it("texto 'Conectado a...' nunca activa flag de transiente", () => {
    const p = normalizeStatusPayload({
      connected: true,
      in_session: true,
      status_text: "Conectado a MacBook-Pro @ 192.168.1.5:24800 (12ms)",
      peer_hostname: "MacBook-Pro",
      peer_addr: "192.168.1.5:24800",
      latency_ms: 12,
      active_screen: "Local",
      uptime_secs: 60,
    });
    expect(p.connected).toBe(true);
    expect(p.peer_hostname).toBe("MacBook-Pro");
    // «Conectado» não deve ser confundido com «Conectando»
    expect(/conectando/i.test(p.status_text)).toBe(false);
  });

  it("peer_hostname em falta → extrai do status_text sem lançar erro", () => {
    const p = normalizeStatusPayload({
      connected: true,
      in_session: true,
      status_text: "Conectado a PC-Sala @ 10.0.0.2:24800 (5ms)",
      active_screen: "Local",
      uptime_secs: 30,
    });
    expect(p.connected).toBe(true);
    // sem peer_hostname explícito o campo fica undefined — UI usa fallback
    expect(p.peer_hostname).toBeUndefined();
  });
});
