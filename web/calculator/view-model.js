// Presentation helpers for the calculator. The Rust engine remains the source
// of truth for all costs; this module decides only what the web page shows.

export const CLOUD_AND_OWN_KEYS = new Set([
  "aws",
  "azure",
  "aws_glacier_ir",
  "azure_cold",
  "on_prem",
]);

export function visibleProviders(response) {
  return response.providers.filter((provider) => CLOUD_AND_OWN_KEYS.has(provider.key));
}

export function providerByKey(response, key) {
  return visibleProviders(response).find((provider) => provider.key === key);
}

export function technicalTotals(response) {
  return visibleProviders(response).map(({ key, name, total_usd, yearly_usd }) => ({
    key,
    name,
    total_usd,
    yearly_usd,
  }));
}
