import { expect, it, vi } from "vitest";
import { createSettingsHttpClient, type SettingsRequest } from "./settings.js";
it("HTTP settings client sends explicit per-request settings and propagates cancellation and errors", async () => {
  const signal = new AbortController().signal;
  const fetcher = vi
    .fn<typeof fetch>()
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ effective: { damping: 0.8 } }), {
        status: 200,
      }),
    )
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ error: "unsupported setting" }), {
        status: 400,
      }),
    );
  const client = createSettingsHttpClient("/prefix/api/", fetcher);
  const request: SettingsRequest = {
    calculation: "mc_ac_pf",
    overrides: { damping: 0.8 },
  };
  expect(await client.resolve(request, signal)).toEqual({
    effective: { damping: 0.8 },
  });
  expect(fetcher).toHaveBeenCalledWith("/prefix/api/settings/resolve", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(request),
    signal,
  });
  await expect(client.resolve(request)).rejects.toThrow(
    "HTTP 400: unsupported setting",
  );
});
