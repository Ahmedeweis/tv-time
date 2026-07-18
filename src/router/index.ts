import { createRouter, createWebHistory } from 'vue-router';
import ListsPage from '../ListsPage.vue';

const routes = [
  {
    path: '/lists',
    name: 'Lists',
    component: ListsPage,
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

export default router;
