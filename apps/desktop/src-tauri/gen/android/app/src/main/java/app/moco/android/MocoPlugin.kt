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
 *   cleared later only if it still holds what we copied.
 */
@TauriPlugin
class MocoPlugin(private val activity: Activity) : Plugin(activity) {
    private val alias = "moco-device-key-v1"
    private var copySeq = 0

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(alias, null) as? SecretKey)?.let { return it }
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setUnlockedDeviceRequired(true)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            init(spec)
            generateKey()
        }
    }

    private fun b64(s: String): ByteArray = Base64.decode(s, Base64.NO_WRAP)
    private fun b64(b: ByteArray): String = Base64.encodeToString(b, Base64.NO_WRAP)

    @Command
    fun protect(invoke: Invoke) {
        try {
            val args = invoke.getArgs()
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key())
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
            val cb = clipboard()
            // The label is ours ("moco-N"); reading it doesn't need clipboard access.
            val label = cb.primaryClipDescription?.label?.toString()
            val cleared = label == "moco-$seq"
            if (cleared) cb.clearPrimaryClip()
            invoke.resolve(JSObject().put("cleared", cleared))
        }
    }
}
