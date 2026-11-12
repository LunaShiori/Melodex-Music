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

  'search.title': '聚合搜索',
  'search.subtitle': '一次输入，同时检索网易云、QQ 音乐与酷我，同曲自动合并。',
  'search.placeholder': '搜索歌曲、歌手，或粘贴歌单链接',
  'search.summary': '{count} 首 · {ms}ms',
  'search.recentKeywords': '最近搜索',
  'search.recentPlays': '最近播放',
  'search.multiSelect': '多选',
  'search.selectAll': '全选',
  'search.downloadAll': '全部下载',
  'search.retry': '重试',
  'search.end': '已到末尾',
  'search.emptyTitle': '搜索你想听的歌',
  'search.emptyDesc': '结果会按音源合并，同一首歌只出现一次，右侧标签表示可从哪些平台取到。',
  'search.emptyHint': '换个关键词，或检查上方的音源是否全部关闭。',
  'search.colSong': '歌曲',
  'search.colSources': '可用音源',
  'search.colQuality': '音质',
  'search.colDuration': '时长',
  'search.sort.default': '默认',
  'search.sort.hot': '热度',
  'search.sort.duration': '时长',
  'search.sort.sources': '音源数',
  'search.sort.defaultHint': '按平台相关度合并后的原始顺序',
  'search.sort.hotHint': '按平台人气值从高到低（没有热度数据的排在后面）',
  'search.sort.durationHint': '短的在前，找试听片段方便',
  'search.sort.sourcesHint': '可下载平台多的在前，成功率更高',

  'source.netease': '网易云音乐',
  'source.qq': 'QQ 音乐',
  'source.kuwo': '酷我音乐',
  'source.migu': '咪咕音乐',
  'source.kugou': '酷狗音乐',
  'source.local': '本地文件',

  'nav.theme.light': '浅色模式',
  'nav.theme.dark': '深色模式',
  'nav.mode.desktop': '桌面端 · 本地引擎',
  'nav.mode.demo': '演示模式 · 内置数据',
  'nav.clearSearch': '清空搜索状态',
}
