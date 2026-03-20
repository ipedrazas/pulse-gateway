export function gatewayStatus(hasTls: boolean, certReady: boolean): "ssl" | "proxy" {
  if (hasTls && certReady) return "ssl";
  return "proxy";
}

export function statusLabel(status: string): string {
  switch (status) {
    case "ssl":
      return "SSL";
    case "proxy":
      return "Proxy";
    default:
      return status;
  }
}

export function fqdn(subdomain: string, domain: string): string {
  return domain ? `${subdomain}.${domain}` : subdomain;
}

export function gatewayUrl(subdomain: string, domain: string, hasTls: boolean): string {
  const proto = hasTls ? "https" : "http";
  return `${proto}://${fqdn(subdomain, domain)}`;
}
