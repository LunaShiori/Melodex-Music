import { createRouter, createWebHashHistory } from 'vue-router'

const router = createRouter({
  // Tauri 用自定义协议，Hash 模式最稳
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/search' },
    {
      path: '/search',
      name: 'search',
      component: () => import('@/views/SearchView.vue'),
      meta: { title: 'nav.search' },
    },
    {
      path: '/recent',
      name: 'recent',
      component: () => import('@/views/RecentView.vue'),
      meta: { title: 'nav.recent' },
    },
    {
      path: '/downloads',
      name: 'downloads',
      component: () => import('@/views/DownloadsView.vue'),
      meta: { title: 'nav.downloads' },
    },
    {
      path: '/library',
      name: 'library',
      component: () => import('@/views/LibraryView.vue'),
      meta: { title: 'nav.library' },
    },
    {
      path: '/playlists',
      name: 'playlists',
      component: () => import('@/views/PlaylistsView.vue'),
      meta: { title: 'nav.playlists' },
    },
    {
      path: '/platforms',
      name: 'platforms',
      component: () => import('@/views/PlatformsView.vue'),
      meta: { title: 'nav.platforms' },
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('@/views/SettingsView.vue'),
      meta: { title: 'nav.settings' },
    },
  ],
})

export default router
