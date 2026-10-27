/**
 * 极简 Markdown 渲染器，只服务「更新说明」这一处。
 *
 * 为什么不引 marked / markdown-it：更新说明是几十行的小文本，引一个 30KB+ 的解析器
 * 再加一个 sanitizer 不划算；而这里的输入来自我们自己的 CHANGELOG（经 GitHub API），
 * 结构固定。先用正则把 HTML 全部转义，再只放行白名单标签，天然没有注入面。
 *
 * 支持：标题、段落、粗体/斜体/删除线、行内代码、链接、有序/无序列表、引用、分割线。
 * 表格与代码块不支持——更新说明里没这两种，真需要时再补。
 */

const ESCAPES: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
}

function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (char) => ESCAPES[char] ?? char)
}

/** 行内标记。传入的是已转义的文本，所以插入的标签不会被二次转义。 */
function inline(text: string): string {
  let out = escapeHtml(text)
  // 行内代码先处理，否则反引号里的 ** 会被当成加粗
  out = out.replace(/`([^`]+)`/g, '<code>$1</code>')
  out = out.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
  out = out.replace(/(^|[^*])\*([^*\n]+)\*/g, '$1<em>$2</em>')
  out = out.replace(/~~([^~]+)~~/g, '<del>$1</del>')
  // 只认 http(s)，其它协议一律当纯文本——发布说明不需要 mailto/file
  out = out.replace(
    /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
    '<a href="$2" data-href="$2" rel="noreferrer noopener">$1</a>',
  )
  // 裸链接也点得动（GitHub 的 release body 里偶尔直接给地址）
  out = out.replace(/(^|[\s（(])(https?:\/\/[^\s<）)]+)/g, '$1<a href="$2" data-href="$2" rel="noreferrer noopener">$2</a>')
  return out
}

export function renderMarkdown(src: string): string {
  const lines = src.replace(/\r\n?/g, '\n').split('\n')
  const html: string[] = []
  let paragraph: string[] = []
  let list: 'ul' | 'ol' | null = null

  const flushParagraph = () => {
    if (!paragraph.length) return
    html.push(`<p class="md-p">${inline(paragraph.join(' '))}</p>`)
    paragraph = []
  }
  const closeList = () => {
    if (!list) return
    html.push(`</${list}>`)
    list = null
  }

  for (const raw of lines) {
    const line = raw.trimEnd()

    if (!line.trim()) {
      flushParagraph()
      closeList()
      continue
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(line.trim())
    if (heading) {
      flushParagraph()
      closeList()
      // 弹窗里最深用到 h5，再深就没意义了
      const level = Math.min(heading[1].length + 2, 5)
      html.push(`<h${level} class="md-h">${inline(heading[2])}</h${level}>`)
      continue
    }

    if (/^(---|\*\*\*|___)\s*$/.test(line.trim())) {
      flushParagraph()
      closeList()
      html.push('<hr class="md-hr" />')
      continue
    }

    const quote = /^>\s?(.*)$/.exec(line.trim())
    if (quote) {
      flushParagraph()
      closeList()
      html.push(`<blockquote class="md-quote">${inline(quote[1])}</blockquote>`)
      continue
    }

    // CHANGELOG 里也用过中点当项目符号，一并认
    const bullet = /^[-*+·]\s+(.*)$/.exec(line.trim())
    if (bullet) {
      flushParagraph()
      if (list !== 'ul') {
        closeList()
        html.push('<ul class="md-ul">')
        list = 'ul'
      }
      html.push(`<li>${inline(bullet[1])}</li>`)
      continue
    }

    const ordered = /^\d+[.)]\s+(.*)$/.exec(line.trim())
    if (ordered) {
      flushParagraph()
      if (list !== 'ol') {
        closeList()
        html.push('<ol class="md-ol">')
        list = 'ol'
      }
      html.push(`<li>${inline(ordered[1])}</li>`)
      continue
    }

    closeList()
    paragraph.push(line.trim())
  }

  flushParagraph()
  closeList()
  return html.join('')
}
