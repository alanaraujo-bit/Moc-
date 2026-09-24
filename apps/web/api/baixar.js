// Redirects to the latest signed Windows installer (read from the updater feed, so the
// site never needs a redeploy for a new version).
const FEED = "https://github.com/alanaraujo-bit/Moc-/releases/latest/download/latest.json";
const FALLBACK = "https://github.com/alanaraujo-bit/Moc-/releases/latest";

export default async function handler(req, res) {
  try {
    const r = await fetch(FEED, { redirect: "follow" });
    const feed = await r.json();
    const url = feed?.platforms?.["windows-x86_64"]?.url;
    res.setHeader("Cache-Control", "public, s-maxage=300, stale-while-revalidate=3600");
    res.redirect(302, url || FALLBACK);
  } catch {
    res.redirect(302, FALLBACK);
  }
}
