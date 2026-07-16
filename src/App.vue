<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
// Since this is a desktop-only app, always assume we're in Tauri!
const isTauri = true;
// Wrapper for invoke (direct call to Tauri)
const invoke = async (cmd: string, args?: any) => {
  console.log(`[App.vue] Calling ${cmd} with args:`, args);
  try {
    const result = await tauriInvoke(cmd, args);
    console.log(`[App.vue] ${cmd} returned:`, result);
    return result;
  } catch (error) {
    console.error(`[App.vue] ${cmd} failed:`, error);
    throw error;
  }
};
// TMDB Auth & Account state
const tmdbSessionId = ref<string | null>(null);
const tmdbAccount = ref<any | null>(null);
const isAuthenticating = ref(false);
const pendingRequestToken = ref<string | null>(null);
interface MediaCache {
  media_id: number;
  title: string;
  poster_path: string | null;
  media_type: string;
  release_date: string | null;
  synopsis_cached_json_data: string;
}
interface UserWatchlistItem {
  media_id: number;
  status: string;
  last_watched_date: string | null;
  notes: string | null;
}
interface WatchlistDashboard {
  released_watchlist: [MediaCache, UserWatchlistItem][];
  upcoming_watchlist: [MediaCache, UserWatchlistItem][];
}
const searchQuery = ref('');
const searchResults = ref<any[]>([]);
const releasedWatchlist = ref<[MediaCache, UserWatchlistItem][]>([]);
const upcomingWatchlist = ref<[MediaCache, UserWatchlistItem][]>([]);
const loading = ref(false);
const search = async () => {
  if (!searchQuery.value.trim()) return;
  loading.value = true;
  console.log('isTauri:', isTauri);
  console.log('Searching for:', searchQuery.value);
  try {
    const results = await invoke('search_tmdb_api', { query: searchQuery.value });
    console.log('Results:', results);
    searchResults.value = results.results || [];
  } catch (error) {
    console.error('Search error:', error);
  } finally {
    loading.value = false;
  }
};
const addToWatchlist = async (mediaId: number, mediaType: string) => {
  try {
    await invoke('add_media_to_watchlist', { mediaId, mediaType });
    if (tmdbSessionId.value) {
      await toggleTmdbWatchlist(mediaId, mediaType, true);
    }
    await loadDashboard();
  } catch (error) {
    console.error('Add to watchlist error:', error);
  }
};
const markAsWatched = async (mediaId: number) => {
  try {
    await invoke('mark_as_watched', { mediaId });
    await loadDashboard();
  } catch (error) {
    console.error('Mark as watched error:', error);
  }
};
const loadDashboard = async () => {
  try {
    const dashboard = await invoke('get_watchlist_dashboard');
    releasedWatchlist.value = dashboard.released_watchlist;
    upcomingWatchlist.value = dashboard.upcoming_watchlist;
  } catch (error) {
    console.error('Load dashboard error:', error);
  }
};
const getDaysUntilRelease = (releaseDate: string | null) => {
  if (!releaseDate) return null;
  const release = new Date(releaseDate);
  const today = new Date();
  const diff = release.getTime() - today.getTime();
  return Math.ceil(diff / (1000 * 3600 * 24));
};
// --- TMDB Account Functions --- //
async function fetchTmdbAccountDetails() {
  try {
    const account = await invoke('get_tmdb_account_details');
    tmdbAccount.value = account;
  } catch (error) {
    console.error('Fetch TMDB account details error:', error);
  }
}
async function checkTmdbSession() {
  try {
    const session = await invoke('get_saved_tmdb_session');
    tmdbSessionId.value = session;
    if (session) {
      await fetchTmdbAccountDetails();
    }
  } catch (error) {
    console.error('Check TMDB session error:', error);
  }
}
async function startTmdbAuth() {
  isAuthenticating.value = true;
  try {
    const token = await invoke('get_tmdb_request_token');
    pendingRequestToken.value = token;
    await invoke('open_tmdb_auth_url', { requestToken: token });
  } catch (error) {
    console.error('Start TMDB auth error:', error);
    isAuthenticating.value = false;
  }
}
async function completeTmdbAuth() {
  if (!pendingRequestToken.value) return;
  try {
    const sessionId = await invoke('create_tmdb_session', { requestToken: pendingRequestToken.value });
    tmdbSessionId.value = sessionId;
    pendingRequestToken.value = null;
    isAuthenticating.value = false;
    await fetchTmdbAccountDetails();
  } catch (error) {
    console.error('Complete TMDB auth error:', error);
    isAuthenticating.value = false;
  }
}
async function toggleTmdbFavorite(mediaId: number, mediaType: string, isFavorite: boolean) {
  try {
    await invoke('add_to_tmdb_favorites', { mediaId, mediaType, favorite: isFavorite });
  } catch (error) {
    console.error('Toggle TMDB favorite error:', error);
  }
}
async function toggleTmdbWatchlist(mediaId: number, mediaType: string, inWatchlist: boolean) {
  try {
    await invoke('add_to_tmdb_watchlist', { mediaId, mediaType, watchlist: inWatchlist });
  } catch (error) {
    console.error('Toggle TMDB watchlist error:', error);
  }
}
onMounted(async () => {
  await checkTmdbSession();
  await loadDashboard();
  try {
    await invoke('get_upcoming_movies', { page: 1 });
  } catch (error) {
    console.error('Get upcoming movies error:', error);
  }
});
</script>
<template>
  <div class="min-h-screen bg-gray-900 text-white p-6">
    <div class="flex items-center justify-between mb-8">
      <h1 class="text-3xl font-bold">Media Tracker</h1>
      <div class="flex items-center gap-4">
        <template v-if="tmdbSessionId && tmdbAccount">
          <div class="flex items-center gap-3 bg-gray-800 px-4 py-2 rounded-lg">
            <img
              v-if="tmdbAccount.avatar?.tmdb?.avatar_path"
              :src="`https://image.tmdb.org/t/p/w45${tmdbAccount.avatar.tmdb.avatar_path}`"
              :alt="tmdbAccount.username"
              class="w-8 h-8 rounded-full"
            />
            <div>
              <p class="text-sm font-semibold">{{ tmdbAccount.name || tmdbAccount.username }}</p>
              <p class="text-xs text-gray-400">@{{ tmdbAccount.username }}</p>
            </div>
          </div>
        </template>
        <template v-else-if="tmdbSessionId">
          <span class="text-green-400 text-sm">✅ Connected to TMDB</span>
        </template>
        <template v-else>
          <button
            @click="startTmdbAuth"
            :disabled="isAuthenticating"
            class="bg-blue-600 hover:bg-blue-700 disabled:opacity-50 px-4 py-2 rounded-lg text-sm"
          >
            {{ isAuthenticating ? 'Authenticating...' : 'Connect to TMDB' }}
          </button>
          <template v-if="pendingRequestToken">
            <button
              @click="completeTmdbAuth"
              class="bg-green-600 hover:bg-green-700 px-4 py-2 rounded-lg text-sm"
            >
              I've Approved the App
            </button>
          </template>
        </template>
      </div>
    </div>
    <div class="mb-10">
      <div class="flex gap-4">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search movies or TV shows..."
          class="flex-1 bg-gray-800 border border-gray-700 rounded-lg px-4 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
          @keyup.enter="search"
        />
        <button
          @click="search"
          :disabled="loading"
          class="bg-blue-600 hover:bg-blue-700 disabled:opacity-50 px-6 py-2 rounded-lg"
        >
          {{ loading ? 'Searching...' : 'Search' }}
        </button>
      </div>
      <div v-if="searchResults.length > 0" class="mt-6 grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
        <div v-for="item in searchResults" :key="item.id" class="bg-gray-800 rounded-lg overflow-hidden">
          <img
            :src="item.poster_path ? `https://image.tmdb.org/t/p/w200${item.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
            :alt="item.title || item.name"
            class="w-full aspect-[2/3] object-cover"
          />
          <div class="p-3">
            <h3 class="text-sm font-semibold truncate">{{ item.title || item.name }}</h3>
            <div class="flex gap-2 mt-2">
              <button
                @click="addToWatchlist(item.id, item.media_type || (item.title ? 'movie' : 'tv'))"
                class="flex-1 bg-green-600 hover:bg-green-700 text-xs py-1 rounded"
              >
                Add to Watchlist
              </button>
              <button
                v-if="tmdbSessionId"
                @click="toggleTmdbFavorite(item.id, item.media_type || (item.title ? 'movie' : 'tv'), true)"
                class="bg-pink-600 hover:bg-pink-700 text-xs px-2 py-1 rounded"
              >
                ❤️
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
    <div class="mb-12">
      <h2 class="text-2xl font-bold mb-4">Released & Watching</h2>
      <div v-if="releasedWatchlist.length > 0" class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
        <div v-for="[media, watchlist] in releasedWatchlist" :key="media.media_id" class="bg-gray-800 rounded-lg overflow-hidden">
          <img
            :src="media.poster_path ? `https://image.tmdb.org/t/p/w200${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
            :alt="media.title"
            class="w-full aspect-[2/3] object-cover"
          />
          <div class="p-3">
            <h3 class="text-sm font-semibold truncate">{{ media.title }}</h3>
            <p class="text-xs text-gray-400">{{ media.release_date }}</p>
            <span
              :class="watchlist.status === 'watched' ? 'bg-green-500' : 'bg-yellow-500'"
              class="inline-block px-2 py-1 mt-2 text-xs rounded"
            >
              {{ watchlist.status }}
            </span>
            <button
              v-if="watchlist.status !== 'watched'"
              @click="markAsWatched(media.media_id)"
              class="mt-2 w-full bg-blue-600 hover:bg-blue-700 text-xs py-1 rounded"
            >
              Mark as Watched
            </button>
          </div>
        </div>
      </div>
      <p v-else class="text-gray-400">No released items in your watchlist yet.</p>
    </div>
    <div>
      <h2 class="text-2xl font-bold mb-4">Upcoming Calendar</h2>
      <div v-if="upcomingWatchlist.length > 0" class="space-y-3">
        <div v-for="[media, watchlist] in upcomingWatchlist" :key="media.media_id" class="flex items-center gap-4 bg-gray-800 p-4 rounded-lg">
          <img
            :src="media.poster_path ? `https://image.tmdb.org/t/p/w100${media.poster_path}` : 'https://via.placeholder.com/100x150?text=No+Image'"
            :alt="media.title"
            class="w-16 aspect-[2/3] object-cover rounded"
          />
          <div class="flex-1">
            <h3 class="font-semibold">{{ media.title }}</h3>
            <p class="text-sm text-gray-400">{{ media.release_date }}</p>
          </div>
          <div class="text-right">
            <span
              v-if="getDaysUntilRelease(media.release_date) !== null"
              class="bg-purple-600 px-3 py-1 rounded-full text-sm"
            >
              {{ getDaysUntilRelease(media.release_date) }} days left
            </span>
          </div>
        </div>
      </div>
      <p v-else class="text-gray-400">No upcoming items in your watchlist yet.</p>
    </div>
  </div>
</template>
