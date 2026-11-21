import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'
import router from './router'
import '@fontsource-variable/noto-sans-sc'
import './styles/main.css'
import './styles/liquid.css'

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.mount('#app')
