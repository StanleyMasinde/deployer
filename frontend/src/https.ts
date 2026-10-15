export type Https = {
  enabled: boolean;
  domain: string | null;
  expiresAt: number | null;
  error: string | null;
};
export function httpsStatus(domain: string, https?: Https, now = Date.now() / 1000) {
  if (!https?.enabled) return "disabled";
  if (https.domain === domain && https.expiresAt !== null)
    return https.expiresAt > now ? "active" : "expired";
  return https.error ? "failed" : "pending";
}
export function appUrl(app: { domain: string; https?: Https }) {
  return `${httpsStatus(app.domain, app.https) === "active" ? "https" : "http"}://${app.domain}`;
}
