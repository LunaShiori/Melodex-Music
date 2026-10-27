#!/usr/bin/env bash
# 一次性配置仓库门面，跑完即可删除令牌。
#
#   export GH_TOKEN=ghp_xxxxxxxx
#   ./scripts/setup-github-meta.sh
#
# 做的事：
#   1. 重命名仓库 melodex → Melodex-Music（重复执行会跳过）
#   2. 写入仓库简介（About → Description）
#   3. 写入 20 个 Topics（决定搜索命中率）
#   4. 把 v0.1.0 标记为 pre-release（它有「点下载即崩溃」的 bug，避免新用户误下）
#
# 令牌要求：classic token 勾选 `repo`（fine-grained 需 Administration: RW
# + Contents: RW 并选中本仓库）。只读令牌会在第一处写操作返回 403。
#
# 不想用令牌也行：网页打开 仓库 → Settings → General → Repository name，
# 改成 Melodex-Music 点 Rename。GitHub 会自动把旧地址 301 到新地址，
# 之后本地执行一次 `git remote set-url origin git@github.com:LunaShiori/Melodex-Music.git`。
set -euo pipefail

: "${GH_TOKEN:?请先 export GH_TOKEN=你的令牌}"
OWNER="LunaShiori"
NAME="Melodex-Music"
OLD_NAME="melodex"
API="https://api.github.com/repos/${OWNER}/${NAME}"
API_OLD="https://api.github.com/repos/${OWNER}/${OLD_NAME}"

DESCRIPTION='Melodex Music｜多源音乐聚合搜索、播放与歌词（Windows + Android）。一个关键词同时搜网易云音乐、QQ音乐、酷我音乐，按「标题+歌手+时长」指纹合并同一首歌并标注每个音源是否可播；内置播放器与逐行歌词（含双语翻译），支持账号歌单、置顶原唱、断点续传、音质自动降级、ID3v2.3 标签与歌词内嵌。Rust + Tauri 2 + Vue 3，Apache-2.0。music player / lyrics / music downloader / music aggregator'

TOPICS='["music","music-player","lyrics","music-downloader","music-search","music-aggregator","music-library","netease-cloud-music","qqmusic","kuwo","lrc-lyrics","id3v2","tauri","tauri-app","rust","vue3","typescript","android","windows","open-source"]'

# 把响应体 + HTTP 码一起返回，方便判断是权限问题还是参数问题
req() {
  local method="$1" url="$2" body="$3"
  curl -sS -w '\n%{http_code}' -X "$method" "$url" \
    -H "Authorization: Bearer ${GH_TOKEN}" \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    -d "$body"
}

report() {
  local label="$1" resp="$2"
  local code body
  code=$(printf '%s' "$resp" | tail -n1)
  body=$(printf '%s' "$resp" | sed '$d')
  if [ "$code" = "200" ] || [ "$code" = "201" ]; then
    echo "  [OK]   ${label}"
  else
    echo "  [FAIL] ${label}  HTTP ${code}"
    printf '%s' "$body" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("         ", d.get("message","(无 message)"))' 2>/dev/null || true
  fi
}

echo "仓库：${OWNER}/${NAME}"

echo "1/4 重命名仓库（${OLD_NAME} → ${NAME}）"
CURRENT=$(curl -sS -H "Authorization: Bearer ${GH_TOKEN}" "${API}" \
  | python3 -c 'import json,sys
d=json.load(sys.stdin)
print(d.get("name") or "")' 2>/dev/null || true)
if [ "${CURRENT}" = "${NAME}" ]; then
  echo "  [SKIP] 已经是 ${NAME}"
else
  # 重命名后 GitHub 会给旧地址留 301，历史下载链接不会断
  report "rename" "$(curl -sS -w '\n%{http_code}' -X PATCH "${API_OLD}" \
    -H "Authorization: Bearer ${GH_TOKEN}" \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    -d "{\"name\":\"${NAME}\"}")"
fi

echo "2/4 写入简介"
report "description" "$(req PATCH "$API" "{\"description\":\"${DESCRIPTION}\"}")"

echo "3/4 写入 Topics"
report "topics" "$(req PUT "${API}/topics" "{\"names\":${TOPICS}}")"

echo "4/4 标记 v0.1.0 为 pre-release"
RID=$(curl -sS -H "Authorization: Bearer ${GH_TOKEN}" "${API}/releases/tags/v0.1.0" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin).get("id",""))')
if [ -z "$RID" ]; then
  echo "  [SKIP] 没找到 v0.1.0 这个 Release"
else
  report "v0.1.0 prerelease" "$(req PATCH "${API}/releases/${RID}" '{"prerelease":true}')"
fi

echo
echo "完成。建议立刻到这里撤销令牌："
echo "  https://github.com/settings/tokens"
