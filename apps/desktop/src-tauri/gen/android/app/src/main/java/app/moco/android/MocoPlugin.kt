package app.moco.android

import android.app.Activity
import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.PersistableBundle
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Android side of Mocó's platform layer (the Windows side is DPAPI + Win32 clipboard).
 *
 * - protect/unprotect: AES-256-GCM with a key that lives in the Android Keystore (hardware
 *   backed where available, never exported). Only usable while the phone is unlocked.
 *   Replaces DPAPI for keeping the Secret Key on this device.
 * - copy/clearIf: clipboard marked sensitive (hidden from the paste preview on Android 13+),
 *   cleared later if it still holds our copy — Android only allows that from the front,
 *   so a clear that comes due in the background runs when the app is back in front.
 */
@TauriPlugin
class MocoPlugin(private val activity: Activity) : Plugin(activity) {
    private val alias = "moco-device-key-v1"
    private var copySeq = 0

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(alias, null) as? SecretKey)?.let { return it }
        // Prefer a key that only works while the phone is unlocked; that needs a secure
        // lock screen, so phones without one get a key that is still hardware-bound.
        return try {
            generate(unlockedOnly = true)
        } catch (e: Exception) {
            generate(unlockedOnly = false)
        }
    }

    private fun generate(unlockedOnly: Boolean): SecretKey {
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setUnlockedDeviceRequired(unlockedOnly)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            init(spec)
            generateKey()
        }
    }

    private fun b64(s: String): ByteArray = Base64.decode(s, Base64.NO_WRAP)
    private fun b64(b: ByteArray): String = Base64.encodeToString(b, Base64.NO_WRAP)

    /** An encrypting cipher; a key the system invalidated (e.g. screen lock removed) is replaced. */
    private fun encryptCipher(): Cipher {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        try {
            cipher.init(Cipher.ENCRYPT_MODE, key())
        } catch (e: Exception) {
            KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.deleteEntry(alias)
            cipher.init(Cipher.ENCRYPT_MODE, key())
        }
        return cipher
    }

    @Command
    fun protect(invoke: Invoke) {
        try {
            val args = invoke.getArgs()
            val cipher = encryptCipher()
            cipher.updateAAD(b64(args.getString("aad")))
            val ct = cipher.doFinal(b64(args.getString("data")))
            invoke.resolve(JSObject().put("blob", b64(cipher.iv + ct)))
        } catch (e: Exception) {
            invoke.reject(e.message ?: "keystore")
        }
    }

    @Command
    fun unprotect(invoke: Invoke) {
        try {
            val args = invoke.getArgs()
            val blob = b64(args.getString("blob"))
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, blob, 0, 12))
            cipher.updateAAD(b64(args.getString("aad")))
            val pt = cipher.doFinal(blob, 12, blob.size - 12)
            invoke.resolve(JSObject().put("data", b64(pt)))
            pt.fill(0)
        } catch (e: Exception) {
            invoke.reject(e.message ?: "keystore")
        }
    }

    private fun clipboard() = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    @Command
    fun copy(invoke: Invoke) {
        val args = invoke.getArgs()
        val sensitive = args.optBoolean("sensitive", true)
        copySeq += 1
        val seq = copySeq
        activity.runOnUiThread {
            try {
                val clip = ClipData.newPlainText("moco-$seq", args.getString("text"))
                if (sensitive) {
                    clip.description.extras = PersistableBundle().apply {
                        val flag = if (Build.VERSION.SDK_INT >= 33) ClipDescription.EXTRA_IS_SENSITIVE else "android.content.extra.IS_SENSITIVE"
                        putBoolean(flag, true)
                    }
                }
                clipboard().setPrimaryClip(clip)
                invoke.resolve(JSObject().put("seq", seq))
            } catch (e: Exception) {
                invoke.reject(e.message ?: "clipboard")
            }
        }
    }

    @Command
    fun clearIf(invoke: Invoke) {
        val seq = invoke.getArgs().getInt("seq")
        activity.runOnUiThread {
            // Android only lets the app in front read or clear the clipboard. In the
            // background we report "not yet" and the app clears when it comes back.
            if (!activity.hasWindowFocus()) {
                invoke.resolve(JSObject().put("cleared", false).put("deferred", true))
                return@runOnUiThread
            }
            val cb = clipboard()
            val cleared = cb.primaryClipDescription?.label?.toString() == "moco-$seq"
            if (cleared) cb.clearPrimaryClip()
            invoke.resolve(JSObject().put("cleared", cleared).put("deferred", false))
        }
    }

    /** Back at the top of the app: leave like Android's home-screen apps do (the lock clock starts). */
    @Command
    fun background(invoke: Invoke) {
        activity.runOnUiThread { activity.moveTaskToBack(true) }
        invoke.resolve()
    }

    /**
     * System bar, cutout and keyboard insets in CSS px (the WebView reports none at the
     * bottom and doesn't resize for the keyboard when drawing edge to edge).
     */
    @Command
    fun insets(invoke: Invoke) {
        activity.runOnUiThread {
            val density = activity.resources.displayMetrics.density
            val all = androidx.core.view.ViewCompat.getRootWindowInsets(activity.window.decorView)
            val bars = all?.getInsets(androidx.core.view.WindowInsetsCompat.Type.systemBars() or androidx.core.view.WindowInsetsCompat.Type.displayCutout())
            val ime = all?.getInsets(androidx.core.view.WindowInsetsCompat.Type.ime())
            invoke.resolve(
                JSObject()
                    .put("top", (bars?.top ?: 0) / density)
                    .put("bottom", (bars?.bottom ?: 0) / density)
                    .put("left", (bars?.left ?: 0) / density)
                    .put("right", (bars?.right ?: 0) / density)
                    .put("keyboard", (ime?.bottom ?: 0) / density)
            )
        }
    }

    /** Status and navigation bar icons dark on a light Mocó, light on a dark one. */
    @Command
    fun barStyle(invoke: Invoke) {
        val dark = invoke.getArgs().optBoolean("dark", false)
        activity.runOnUiThread {
            val c = androidx.core.view.WindowCompat.getInsetsController(activity.window, activity.window.decorView)
            c.isAppearanceLightStatusBars = !dark
            c.isAppearanceLightNavigationBars = !dark
            invoke.resolve()
        }
    }

    private var webView: android.webkit.WebView? = null

    override fun load(webView: android.webkit.WebView) {
        this.webView = webView
    }

    /** The system print dialog for the page's print layout ("Salvar como PDF" included). */
    @Command
    fun print(invoke: Invoke) {
        val title = invoke.getArgs().optString("title", "Mocó")
        activity.runOnUiThread {
            val wv = webView
            if (wv == null) {
                invoke.reject("impressão indisponível")
                return@runOnUiThread
            }
            val pm = activity.getSystemService(Context.PRINT_SERVICE) as android.print.PrintManager
            val attrs = android.print.PrintAttributes.Builder().setMediaSize(android.print.PrintAttributes.MediaSize.ISO_A4).build()
            pm.print(title, wv.createPrintDocumentAdapter(title), attrs)
            invoke.resolve()
        }
    }

    // ---- Biometric unlock ----------------------------------------------------------------
    //
    // Same shape as Windows Hello on the desktop: a Keystore HMAC key that only works right
    // after a strong biometric (fingerprint/face class 3), is invalidated when a new
    // biometric is enrolled, and never leaves the secure hardware. The prompt releases it
    // through a CryptoObject and it MACs a fixed per-account challenge; the result derives
    // the key that unwraps the Account Key (hello.rs). No biometric, no key — not a yes/no.

    private fun bioStatus(): Int =
        androidx.biometric.BiometricManager.from(activity)
            .canAuthenticate(androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG)

    @Command
    fun bioAvailable(invoke: Invoke) {
        val s = bioStatus()
        invoke.resolve(
            JSObject()
                .put("available", s == androidx.biometric.BiometricManager.BIOMETRIC_SUCCESS)
                .put("hardware", s != androidx.biometric.BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE && s != androidx.biometric.BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE)
        )
    }

    private fun createMacKey(alias: String) {
        val b = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(true)
        if (Build.VERSION.SDK_INT >= 30) {
            b.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
        } else {
            @Suppress("DEPRECATION")
            b.setUserAuthenticationValidityDurationSeconds(-1)
        }
        KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_HMAC_SHA256, "AndroidKeyStore").run {
            init(b.build())
            generateKey()
        }
    }

    @Command
    fun bioMac(invoke: Invoke) {
        val args = invoke.getArgs()
        val alias = args.getString("alias")
        val data = b64(args.getString("data"))
        val create = args.optBoolean("create", false)
        val title = args.optString("title", "Abrir o Mocó")
        val subtitle = args.optString("subtitle", "")
        activity.runOnUiThread {
            val mac: javax.crypto.Mac
            try {
                val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
                if (create) {
                    ks.deleteEntry(alias)
                    createMacKey(alias)
                }
                val key = ks.getKey(alias, null) as? SecretKey
                if (key == null) {
                    invoke.reject("A biometria não está ativada neste celular.", "missing")
                    return@runOnUiThread
                }
                mac = javax.crypto.Mac.getInstance("HmacSHA256")
                mac.init(key)
            } catch (e: android.security.keystore.KeyPermanentlyInvalidatedException) {
                invoke.reject("Uma digital ou rosto novo foi cadastrado no celular. Use a senha mestra e ative a biometria de novo.", "invalidated")
                return@runOnUiThread
            } catch (e: Exception) {
                invoke.reject(e.message ?: "keystore", "failed")
                return@runOnUiThread
            }
            val prompt = androidx.biometric.BiometricPrompt(
                activity as androidx.fragment.app.FragmentActivity,
                androidx.core.content.ContextCompat.getMainExecutor(activity),
                object : androidx.biometric.BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: androidx.biometric.BiometricPrompt.AuthenticationResult) {
                        try {
                            val out = result.cryptoObject!!.mac!!.doFinal(data)
                            invoke.resolve(JSObject().put("mac", b64(out)))
                            out.fill(0)
                        } catch (e: Exception) {
                            invoke.reject(e.message ?: "mac", "failed")
                        }
                    }

                    override fun onAuthenticationError(code: Int, msg: CharSequence) {
                        val c = when (code) {
                            androidx.biometric.BiometricPrompt.ERROR_USER_CANCELED,
                            androidx.biometric.BiometricPrompt.ERROR_NEGATIVE_BUTTON,
                            androidx.biometric.BiometricPrompt.ERROR_CANCELED -> "canceled"
                            androidx.biometric.BiometricPrompt.ERROR_LOCKOUT,
                            androidx.biometric.BiometricPrompt.ERROR_LOCKOUT_PERMANENT -> "locked"
                            else -> "failed"
                        }
                        invoke.reject(msg.toString(), c)
                    }
                    // A wrong finger just lets the person try again inside the prompt.
                },
            )
            val info = androidx.biometric.BiometricPrompt.PromptInfo.Builder()
                .setTitle(title)
                .apply { if (subtitle.isNotEmpty()) setSubtitle(subtitle) }
                .setNegativeButtonText("Usar a senha mestra")
                .setAllowedAuthenticators(androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG)
                .setConfirmationRequired(false)
                .build()
            prompt.authenticate(info, androidx.biometric.BiometricPrompt.CryptoObject(mac))
        }
    }

    @Command
    fun bioRemove(invoke: Invoke) {
        try {
            KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.deleteEntry(invoke.getArgs().getString("alias"))
        } catch (e: Exception) {}
        invoke.resolve()
    }
}
