// Redirects to the latest signed Windows installer (read from the updater feed, so the
// site never needs a redeploy for a new version). Android phones get the APK instead.
const FEED = "https://github.com/alanaraujo-bit/Moc-/releases/latest/download/latest.json";
const FALLBACK = "https://github.com/alanaraujo-bit/Moc-/releases/latest";

export default async function handler(req, res) {
  if (/Android/i.test(req.headers["user-agent"] ?? "")) {
    res.setHeader("Cache-Control", "private, no-store");
    res.redirect(302, "/baixar/android");
    return;
  }
  try {
    const r = await fetch(FEED, { redirect: "follow" });
    const feed = await r.json();
    const url = feed?.platforms?.["windows-x86_64"]?.url;
    // Varies by device: don't let a shared cache hand phones the .exe.
    res.setHeader("Cache-Control", "private, max-age=300");
    res.redirect(302, url || FALLBACK);
  } catch {
    res.redirect(302, FALLBACK);
  }
}
