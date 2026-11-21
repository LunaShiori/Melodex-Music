<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { recent, loadHistory } from '@/lib/history'
import Icon from '@/components/Icon.vue'
import { usePlayerStore } from '@/stores/player'
import { useFavorites } from '@/stores/favorites'
const player=usePlayerStore(), favorites=useFavorites()
loadHistory()
const router=useRouter(), query=ref('')
const links=[{path:'/favorites',icon:'heart',title:'收藏与歌单'},{path:'/platforms',icon:'external',title:'发现夹'},{path:'/downloads',icon:'download',title:'下载中心'},{path:'/settings',icon:'settings',title:'设置'}]
const title=ref(''), notice=ref('')
const resume=computed(()=>{try{return JSON.parse(localStorage.getItem('melodex.resume.v1')||'null')}catch{return null}})
const cover=computed(()=>player.current?.cover || resume.value?.queue?.[resume.value.index]?.cover || favorites.tracks.find(t=>t.cover)?.cover)
function saveQueue(){try{favorites.saveList(title.value.trim(),player.queue);notice.value='队列已存入歌单';title.value=''}catch{notice.value='保存失败，请检查设备空间'}}
</script>
<template>
  <main class="edition">
    <section class="lead">
      <div class="lead-copy">
        <h1>{{ player.current?.title || resume?.queue?.[resume.index]?.title || '今天，听点什么？' }}</h1>
        <button v-if="player.current" class="btn" @click="player.toggle()">{{player.playing?'暂停播放':'继续播放'}}</button>
        <button v-else-if="resume" class="btn" @click="player.restoreSession()">从上次的位置继续</button>
        <form class="home-search" @submit.prevent="router.push({path:'/search',query:{q:query.trim()}})">
          <Icon name="search" :size="18"/><input v-model="query" type="search" aria-label="搜索音乐" placeholder="歌曲、歌手或专辑"/><button type="submit">搜索</button>
        </form>
      </div>
      <img v-if="cover" :src="cover" alt=""/><div v-else class="record-art" aria-hidden="true"><i/></div>
    </section>
    <nav class="launchers" aria-label="我的音乐"><RouterLink v-for="link in links" :key="link.path" :to="link.path"><Icon :name="link.icon" :size="20"/><span>{{link.title}}</span></RouterLink></nav>
    <div class="home-columns" :class="{queued:player.queue.length}">
      <div v-if="recent.length || favorites.lists.length" class="home-shelves">
        <section v-if="recent.length"><header><h2>最近播放</h2><RouterLink to="/recent">查看全部</RouterLink></header>
          <button v-for="(item,i) in recent.slice(0,5)" :key="item.fingerprint" class="home-song" @click="player.playList([...recent],i)"><Icon name="play" :size="16"/><strong>{{item.title}}</strong><span>{{item.artists.join(' / ')}}</span></button>
        </section>
        <section v-if="favorites.lists.length"><h2>我的歌单</h2><button v-for="list in favorites.lists" :key="list.name" class="home-song" @click="player.playList([...list.tracks])"><Icon name="queue" :size="16"/><strong>{{list.name}}</strong><span>{{list.tracks.length}} 首</span></button></section>
      </div>
      <aside v-if="player.queue.length"><h2>播放队列</h2><button v-for="(item,i) in player.queue" :key="i" class="home-song" @click="player.jumpTo(i)"><span>{{i+1}}</span><strong>{{item.title}}</strong></button>
        <form @submit.prevent="saveQueue"><input v-model="title" aria-label="歌单名称" placeholder="歌单名称" maxlength="80"/><button class="btn" :disabled="!title.trim()">保存队列</button></form><p role="status">{{notice}}</p>
      </aside>
    </div>
  </main>
</template>
<style scoped>.edition{padding:clamp(16px,4vw,56px);max-width:1360px;margin:auto}.lead{background:#28473f;color:#fff4da;min-height:230px;display:flex;align-items:center;justify-content:space-between;gap:16px;padding:clamp(22px,5vw,64px);border-radius:20px;overflow:hidden}.lead h1{font-family:Georgia,serif;font-size:clamp(32px,5vw,76px);line-height:1.04;margin:0 0 28px;overflow-wrap:anywhere}.lead img{width:30%;aspect-ratio:1;object-fit:cover;border-radius:5px}.desk{display:grid;grid-template-columns:3fr 2fr;gap:28px}.desk aside{border-left:1px solid var(--line);padding-left:24px}.desk h2{margin:24px 0 12px}.desk nav a,.line{display:flex;justify-content:space-between;width:100%;padding:12px 0;border:0;border-bottom:1px solid var(--line);color:inherit;text-decoration:none;background:none;text-align:left}.desk small{border:1px solid;padding:1px 5px;border-radius:4px}.desk input{max-width:100%}@media(max-width:700px){.desk{grid-template-columns:1fr;gap:0}.desk aside{border:0;padding:0}.lead{min-height:180px}}</style>