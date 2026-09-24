/** Where Mocó is running. The same frontend ships on Windows and Android. */
export const isAndroid = /Android/i.test(navigator.userAgent);
export const isMobile = isAndroid;

/** "computador" or "celular", for copy that names this device. */
export const deviceNoun = isMobile ? "celular" : "computador";
