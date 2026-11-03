// 由 scripts/android-apk-plugin.py 复制进 gen/android 工程；改这里就够了。
// package 行会被脚本按 tauri.conf.json 的 identifier 重写。
package app.melodex

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Intent
import android.content.pm.PackageManager
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File
import java.util.zip.ZipFile

@InvokeArg
class ApkInstallArgs {
  var path: String? = null
}

/**
 * 把下载好的 APK 交给系统安装器。
 *
 * 为什么不用现成的 opener 插件：
 * 1. 它的 `open_path` 在移动端把路径当裸字符串发过来，Kotlin 侧要的是 `{ url }` 结构，
 *    Jackson 直接报 “no String-argument constructor/factory method to deserialize”；
 * 2. 就算把结构拼对，`file://` 从 Android 7 起也会被 StrictMode 拦掉（FileUriExposedException），
 *    必须换成自己 FileProvider 提供的 `content://`，并带上安装器的 MIME 类型，
 *    系统才会理会「这是一个 APK，去装它」。
 *
 * 拉起安装器之前先自己把包验一遍：文件不完整、下到的是代理的错误页时，
 * 系统只会甩一句「解析软件包时出现问题」，用户完全不知道该怎么办。
 * 这里提前拦下并说清原因，前端才好给「重新下载」的入口。
 */
@TauriPlugin
class ApkInstallPlugin(private val activity: Activity) : Plugin(activity) {
  /** 常见系统安装器的包名：queryIntentActivities 拿不到时兜底授权 */
  private val KNOWN_INSTALLERS = listOf(
    "com.miui.packageinstaller",
    "com.android.packageinstaller",
    "com.google.android.packageinstaller",
    "com.samsung.android.packageinstaller"
  )

  /** 4 字节的 ZIP 本地文件头魔数；APK 本质就是个 ZIP */
  private val zipMagic = byteArrayOf(0x50, 0x4B, 0x03, 0x04)

  private fun looksLikeApk(file: File): Boolean {
    if (file.length() < 1024) return false
    val head = ByteArray(4)
    file.inputStream().use { stream ->
      if (stream.read(head) != 4) return false
    }
    if (!head.contentEquals(zipMagic)) return false
    // 能当 ZIP 打开、且里面有 AndroidManifest.xml，才算一个可解析的安装包
    return try {
      ZipFile(file).use { zip -> zip.getEntry("AndroidManifest.xml") != null }
    } catch (ex: Exception) {
      false
    }
  }

  @Command
  fun install(invoke: Invoke) {
    try {
      val path = invoke.parseArgs(ApkInstallArgs::class.java).path
        ?: throw IllegalArgumentException("缺少安装包路径")
      val file = File(path)
      if (!file.exists()) {
        throw IllegalArgumentException("安装包不存在，请重新下载")
      }
      if (!looksLikeApk(file)) {
        val mb = file.length() / 1024.0 / 1024.0
        throw IllegalArgumentException(
          "安装包不完整（%.1f MB），系统无法解析。请点「重新下载」再试一次".format(mb)
        )
      }
      val authority = "${activity.packageName}.apkprovider"
      val uri = FileProvider.getUriForFile(activity, authority, file)
      val readGrant = Intent.FLAG_GRANT_READ_URI_PERMISSION
      val intent = Intent(Intent.ACTION_VIEW).apply {
        setDataAndType(uri, "application/vnd.android.package-archive")
        addFlags(readGrant)
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        // 有些系统安装器（小米的 packageinstaller 就是）不在自己的进程里直接读这个
        // content://，而是把 URI 转交给另一个进程再打开。只靠 intent 上的
        // FLAG_GRANT_READ_URI_PERMISSION，那次转交就拿不到授权，用户看到的是
        // 「解析软件包时出现问题 (11) Permission Denial: ... not exported」。
        // 所以这里做两件事：URI 同时放进 ClipData，并给每个能处理它的应用显式授权。
        clipData = ClipData.newRawUri("apk", uri)
      }
      val handlers = activity.packageManager
        .queryIntentActivities(intent, PackageManager.MATCH_DEFAULT_ONLY)
      for (handler in handlers) {
        activity.grantUriPermission(handler.activityInfo.packageName, uri, readGrant)
      }
      // 系统 installer 常常以 resolver 身份出现、不会出现在上面的列表里（Android 11+
      // 的包可见性还会把 queryIntentActivities 过滤空），所以再按常见包名逐个补授权；
      // 包不存在时这个调用是空操作，不会抛异常。
      for (pkg in KNOWN_INSTALLERS) {
        activity.grantUriPermission(pkg, uri, readGrant)
      }
      try {
        activity.startActivity(intent)
      } catch (ex: ActivityNotFoundException) {
        throw IllegalStateException(
          "系统里没有能安装 APK 的应用，请到「设置 → 应用 → 安装未知应用」里检查后重试"
        )
      }
      invoke.resolve()
    } catch (ex: Exception) {
      // 最常见的是「不允许安装未知应用」：系统会自己弹授权页，这里只把原因回给前端
      invoke.reject(ex.message ?: "无法打开安装包")
    }
  }
}
