// 由 scripts/android-apk-plugin.py 复制进 gen/android 工程；改这里就够了。
// package 行会被脚本按 tauri.conf.json 的 identifier 重写。
package app.melodex

import android.app.Activity
import android.content.Intent
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File

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
 */
@TauriPlugin
class ApkInstallPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun install(invoke: Invoke) {
    try {
      val path = invoke.parseArgs(ApkInstallArgs::class.java).path
        ?: throw IllegalArgumentException("缺少安装包路径")
      val file = File(path)
      if (!file.exists()) {
        throw IllegalArgumentException("安装包不存在，请重新下载")
      }
      val authority = "${activity.packageName}.apkprovider"
      val uri = FileProvider.getUriForFile(activity, authority, file)
      val intent = Intent(Intent.ACTION_VIEW).apply {
        setDataAndType(uri, "application/vnd.android.package-archive")
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      }
      activity.startActivity(intent)
      invoke.resolve()
    } catch (ex: Exception) {
      // 最常见的是「不允许安装未知应用」：系统会自己弹授权页，这里只把原因回给前端
      invoke.reject(ex.message ?: "无法打开安装包")
    }
  }
}
