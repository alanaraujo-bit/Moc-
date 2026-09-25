// On an Android phone, the Android download is the main one.
if (/Android/i.test(navigator.userAgent)) {
  for (const cta of document.querySelectorAll(".cta")) {
    const android = cta.querySelector('a[href="/baixar/android"]');
    const windows = cta.querySelector('a[href="/baixar"]');
    if (!android || !windows) continue;
    cta.insertBefore(android, windows);
    android.classList.remove("alt");
    windows.classList.add("alt");
  }
}
