// Redirects to the newest Android APK among the GitHub releases (Windows releases may come
// out without one, so "latest" alone isn't enough).
const RELEASES = "https://api.github.com/repos/alanaraujo-bit/Moc-/releases?per_page=15";
const FALLBACK = "https://github.com/alanaraujo-bit/Moc-/releases";

export default async function handler(req, res) {
  try {
    const r = await fetch(RELEASES, { headers: { Accept: "application/vnd.github+json", "User-Agent": "moco-site" } });
    const releases = await r.json();
    const apk = (Array.isArray(releases) ? releases : [])
      .filter((rel) => !rel.draft && !rel.prerelease) // stable only; betas live in the app's beta channel
      .flatMap((rel) => rel.assets ?? [])
      .find((a) => a.name.endsWith(".apk"));
    res.setHeader("Cache-Control", "public, s-maxage=600, stale-while-revalidate=86400");
    res.redirect(302, apk?.browser_download_url || FALLBACK);
  } catch {
    res.redirect(302, FALLBACK);
  }
}
