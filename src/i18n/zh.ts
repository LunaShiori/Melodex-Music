/**
 * 中文词条（默认语言）。
 *
 * 键名用点号分级：`nav.search` 表示界面导航里的「搜索」。
 * 占位符写成 `{name}`，由 `t()` 负责替换。
 * 新增词条时必须同时在 `en.ts` 里补一条，缺词条会回落到这里。
 */
export const zh: Record<string, string> = {
  // 导航
  'nav.search': '搜索',
  'nav.recent': '最近',
  'nav.downloads': '下载',
  'nav.library': '曲库',
  'nav.playlists': '歌单',
  'nav.settings': '设置',
  'nav.sources': '音源',

  // 通用
  'common.back': '返回',
  'common.search': '搜索',
  'common.searching': '搜索中',
  'common.download': '下载',
  'common.cancel': '取消',
  'common.confirm': '确定',
  'common.clear': '清空',
  'common.refresh': '刷新',
  'common.more': '加载更多',
  'common.unknownArtist': '未知歌手',
  'common.empty': '这里还没有内容',

  // 播放器
  'player.prev': '上一首',
  'player.next': '下一首',
  'player.play': '播放',
  'player.pause': '暂停',
  'player.queue': '播放队列',
  'player.lyrics': '歌词详情',
  'player.mute': '静音',
  'player.volume': '音量 {percent}%',
  'player.openLyrics': '打开歌词详情',
  'player.mode.order': '顺序播放',
  'player.mode.repeatOne': '单曲循环',
  'player.mode.shuffle': '随机播放',
  'player.modeHint': '{mode}（点击切换）',
  'player.idle': '未在播放',
  'player.idleHint': '从搜索结果里点一下播放',
  'player.demo': 'Melodex 演示模式',

  // 设置：语言
  'settings.language.title': '语言',
  'settings.language.hint': '界面语言。选「跟随系统」时按系统语言自动切换。',
  'settings.language.auto': '跟随系统',
  'settings.language.zh': '简体中文',
  'settings.language.en': 'English',
}
