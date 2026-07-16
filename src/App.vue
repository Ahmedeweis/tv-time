<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import moviesIcon from './assets/movies.png';
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
// Logo SVG
const logoSvg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' rx='16' fill='#545454'/><rect x='20' y='20' width='20' height='20' fill='#fdeb69'/><rect x='40' y='20' width='20' height='20' fill='#fce45e'/><rect x='60' y='20' width='20' height='20' fill='#f3d457'/><rect x='40' y='40' width='20' height='20' fill='#f9d457'/><rect x='40' y='60' width='20' height='20' fill='#efbe4e'/></svg>`;
const logoUrl = `data:image/svg+xml;utf8,${encodeURIComponent(logoSvg)}`;
// TMDB Auth & Account state
const tmdbSessionId = ref<string | null>(null);
const tmdbAccount = ref<any | null>(null);
const isAuthenticating = ref(false);
const pendingRequestToken = ref<string | null>(null);
// Page state for bottom nav
const currentPage = ref('home');
const activeTab = ref('upcoming');
const selectedMedia = ref<any>(null);
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
const watchlistMediaIds = computed(() => {
  const ids = new Set<number>();
  releasedWatchlist.value.forEach(([media]) => ids.add(media.media_id));
  upcomingWatchlist.value.forEach(([media]) => ids.add(media.media_id));
  return ids;
});
const unwatchedReleased = computed(() => {
  return releasedWatchlist.value.filter(([_, watchlist]) => watchlist.status !== 'watched');
});
const isWatched = (mediaId: number) => {
  const entry = [...releasedWatchlist.value, ...upcomingWatchlist.value].find(([m]) => m.media_id === mediaId);
  return entry ? entry[1].status === 'watched' : false;
};
const loading = ref(false);
const getRandomCount = () => {
  const min = 79;
  const max = 700000;
  const num = Math.floor(Math.random() * (max - min + 1)) + min;
  if (num >= 1000) {
    return (num / 1000).toFixed(1).replace(/\.0$/, '') + 'k';
  }
  return num.toString();
};
const openMediaDetails = (item: any, mediaType?: string) => {
  selectedMedia.value = {
    ...item,
    media_type: mediaType || item.media_type || (item.title ? 'movie' : 'tv')
  };
  currentPage.value = 'details';
};
const search = async () => {
  if (!searchQuery.value.trim()) return;
  loading.value = true;
  console.log('isTauri:', isTauri);
  console.log('Searching for:', searchQuery.value);
  try {
    const results = await invoke('search_tmdb_api', { query: searchQuery.value });
    console.log('Results:', results);
    searchResults.value = (results.results || []).map((item: any) => ({
      ...item,
      randomCount: getRandomCount(),
    }));
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
  <div class="min-h-screen bg-white text-black p-6 pb-24" style="padding-top: 0;">
    <!-- <div class="flex items-center justify-between mb-8">
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
    </div> -->
    <!-- Home Page -->
    <div v-if="currentPage === 'home'">
      <!-- Tabs -->
      <div class="flex justify-around items-center mb-6 border-b-2 border-gray-200">
        <button
          @click="activeTab = 'upcoming'"
          class="pb-2 px-6 text-xl font-bold transition-colors"
          :class="activeTab === 'upcoming' ? 'text-yellow-400 border-b-4 border-yellow-400' : 'text-gray-400'"
        >
          المرتقبة
        </button>
        <button
          @click="activeTab = 'unwatched'"
          class="pb-2 px-6 text-xl font-bold transition-colors"
          :class="activeTab === 'unwatched' ? 'text-yellow-400 border-b-4 border-yellow-400' : 'text-gray-400'"
        >
          قائمة المشاهدة
        </button>
      </div>
      <!-- Upcoming Tab Content -->
      <div v-if="activeTab === 'upcoming'">
        <div v-if="upcomingWatchlist.length > 0" class="grid grid-cols-3 gap-4">
          <div
            v-for="[media, _] in upcomingWatchlist"
            :key="media.media_id"
            class="flex flex-col items-center cursor-pointer hover:opacity-80"
            @click="openMediaDetails(media)"
          >
            <img
              :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
              :alt="media.title"
              class="w-full aspect-[2/3] object-cover rounded-lg"
            />
            <div class="mt-2 flex flex-col items-center">
              <span class="text-2xl font-bold text-black">
                {{ getDaysUntilRelease(media.release_date) }}
              </span>
              <span class="text-lg font-semibold text-black">
                أيام
              </span>
            </div>
          </div>
        </div>
        <p v-else class="text-gray-400 text-center">No upcoming items yet.</p>
      </div>
      <!-- Unwatched Tab Content -->
      <div v-else-if="activeTab === 'unwatched'">
        <div v-if="unwatchedReleased.length > 0" class="grid grid-cols-3 gap-4">
          <div
            v-for="[media, watchlist] in unwatchedReleased"
            :key="media.media_id"
            class="flex flex-col items-center cursor-pointer hover:opacity-80"
            @click="openMediaDetails(media)"
          >
            <img
              :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
              :alt="media.title"
              class="w-full aspect-[2/3] object-cover rounded-lg"
            />
            <div class="mt-2 flex flex-col items-center">
              <button
                @click.stop="markAsWatched(media.media_id)"
                class="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-lg text-sm"
              >
                Mark as Watched
              </button>
            </div>
          </div>
        </div>
        <p v-else class="text-gray-400 text-center">No unwatched items yet.</p>
      </div>
    </div>
    <!-- Discover Page (new) -->
    <div v-else-if="currentPage === 'discover'">
      <div class="mb-6">
        <div class="flex gap-4 items-center">
          <div class="flex-1 relative">
            <!-- Search Icon on Left -->
            <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <circle cx="11" cy="11" r="8" stroke-width="2"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65" stroke-width="2"/>
            </svg>
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Search movies or TV shows..."
              class="w-full bg-white text-black border border-gray-300 rounded-lg pl-10 pr-10 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
              @keyup.enter="search"
            />
            <!-- Clear (X) Icon on Right -->
            <button
              v-if="searchQuery"
              @click="searchQuery = ''"
              class="absolute right-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-500 hover:text-gray-700"
            >
              <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <line x1="18" y1="6" x2="6" y2="18" stroke-width="2"/>
                <line x1="6" y1="6" x2="18" y2="18" stroke-width="2"/>
              </svg>
            </button>
          </div>
          <button
            @click="search"
            :disabled="loading"
            class="bg-blue-600 hover:bg-blue-700 disabled:opacity-50 px-6 py-2 rounded-lg text-white"
          >
            {{ loading ? 'Searching...' : 'Search' }}
          </button>
          <!-- Cancel Button to go back home -->
          <button
            @click="currentPage = 'home'; searchQuery = ''"
            class="text-gray-700 hover:text-gray-900 font-medium"
          >
            Cancel
          </button>
        </div>
      </div>
      <div v-if="searchResults.length > 0" class="space-y-3">
        <div
          v-for="item in searchResults"
          :key="item.id"
          class="flex items-center gap-4 cursor-pointer hover:bg-gray-50 p-4"
          @click="openMediaDetails(item)"
        >
          <img
            :src="item.poster_path ? `https://image.tmdb.org/t/p/w92${item.poster_path}` : 'https://via.placeholder.com/100x150?text=No+Image'"
            :alt="item.title || item.name"
            class="w-16 aspect-[2/3] object-cover rounded"
            @error="console.log('Image load error:', item.poster_path, $event)"
          />
          <div class="flex-1 pl-4">
            <div class="flex items-center gap-2">
              <h3 class="font-semibold text-lg">{{ item.title || item.name }}</h3>
            </div>
            <div class="flex items-center gap-2">
              <img :src="moviesIcon" alt="Movie icon" class="w-5 h-5" />
              <p class="text-sm text-black">{{ item.randomCount }} added this show</p>
              <p
                class="text-xl font-bold"
                :class="(item.media_type || (item.title ? 'movie' : 'tv')) === 'movie' ? 'text-red-600' : 'text-green-600'"
              >
                {{ item.media_type ? (item.media_type === 'movie' ? 'Movie' : 'TV Show') : (item.title ? 'Movie' : 'TV Show') }}
              </p>
            </div>
          </div>
          <button
            v-if="!watchlistMediaIds.has(item.id)"
            @click.stop="addToWatchlist(item.id, item.media_type || (item.title ? 'movie' : 'tv'))"
            class="w-12 h-12 flex items-center justify-center bg-white border-2 border-yellow-400 rounded-lg hover:bg-yellow-100"
          >
            <svg class="w-6 h-6 text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <line x1="12" y1="5" x2="12" y2="19" stroke-width="2"/>
              <line x1="5" y1="12" x2="19" y2="12" stroke-width="2"/>
            </svg>
          </button>
          <button
            v-else
            class="w-12 h-12 flex items-center justify-center bg-yellow-400 rounded-lg"
            disabled
          >
            <svg class="w-6 h-6 text-black" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <polyline points="20 6 9 17 4 12" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></polyline>
            </svg>
          </button>
        </div>
      </div>
    </div>
    <!-- Media Details Page -->
    <div v-else-if="currentPage === 'details' && selectedMedia" class="pb-24 -mx-6">
      <div class="relative">
        <img
          :src="selectedMedia.backdrop_path ? `https://image.tmdb.org/t/p/w1280${selectedMedia.backdrop_path}` : (selectedMedia.poster_path ? `https://image.tmdb.org/t/p/w780${selectedMedia.poster_path}` : 'https://via.placeholder.com/1280x720?text=No+Image')"
          :alt="selectedMedia.title || selectedMedia.name"
          class="w-full aspect-[9/16] md:aspect-[16/9] object-cover"
        />
        <div class="absolute inset-0 bg-gradient-to-t from-black via-transparent to-transparent" />
        <button
          @click="currentPage = 'home'"
          class="absolute top-20 left-4 p-2 bg-black/30 rounded-full"
        >
          <svg class="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
          </svg>
        </button>
        <div class="absolute top-20 right-4 flex  gap-2 cursor-pointer">
          <div class="w-2 h-2 bg-white rounded-full" />
          <div class="w-2 h-2 bg-white/50 rounded-full" />
          <div class="w-2 h-2 bg-white/50 rounded-full" />
        </div>
        <div class="absolute bottom-0 left-0 right-0 p-6">
          <h1 class="text-3xl font-bold text-white mb-2">
            {{ selectedMedia.title || selectedMedia.name }}
          </h1>
          <p class="text-white/90 text-lg mb-2">
            {{ selectedMedia.runtime ? `${Math.floor(selectedMedia.runtime / 60)}h ${selectedMedia.runtime % 60}m` : '2h 49m' }} • {{ selectedMedia.genres?.map(g => g.name).join(', ') || 'Action, Adventure, Thriller' }}
          </p>
        </div>
      </div>
      <div class="px-6 pt-6 space-y-4">
        <div class="flex items-center justify-between gap-4 text-gray-500">
 <div class="flex items-center justify-start gap-4">
           <div class="flex items-center gap-2">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <rect x="3" y="4" width="18" height="18" rx="2" ry="2" stroke-width="2"/>
              <line x1="16" y1="2" x2="16" y2="6" stroke-width="2"/>
              <line x1="8" y1="2" x2="8" y2="6" stroke-width="2"/>
              <line x1="3" y1="10" x2="21" y2="10" stroke-width="2"/>
            </svg>
            <span>{{ selectedMedia.release_date || selectedMedia.first_air_date || 'May 17, 2025' }}</span>
          </div>
          <div class="flex items-center gap-2">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"/>
            </svg>
            <span>{{ isWatched(selectedMedia.id) ? 'Watched' : 'Not watched' }}</span>
          </div>
 </div>
          <button
            v-if="watchlistMediaIds.has(selectedMedia.id)"
            @click="markAsWatched(selectedMedia.id)"
            :class="[
              'w-8 h-8 rounded-full flex items-center justify-center',
              isWatched(selectedMedia.id) ? 'bg-green-500' : 'bg-gray-300'
            ]"
          >
            <svg
              :class="[
                'w-5 h-5',
                isWatched(selectedMedia.id) ? 'text-white' : 'text-black'
              ]"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <polyline points="20 6 9 17 4 12" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
        </div>
        <div class="flex border-b border-gray-200 justify-between w-full">
          <button class="pb-2 px-6 border-b-2 border-black align-center text-center font-semibold w-[1/2]">
            ABOUT
          </button>
          <button class="pb-2 px-6 text-gray-500 align-center text-center font-center w-[1/2]">
            MORE
          </button>
        </div>
        <div class="space-y-4">
          <div class="flex justify-between items-center">
            <p class="text-gray-600">Where to watch</p>
            <span class="text-gray-400">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <circle cx="10.5" cy="10.5" r="7.5" stroke-width="2"/>
                <path d="M16.5 16.5L21 21" stroke-width="2" stroke-linecap="round"/>
              </svg>
            </span>
          </div>
          <p class="text-gray-500">Not available</p>
          <div>
            <p class="text-gray-600 mb-2">Movie info</p>
            <div class="flex items-center gap-2 mb-2">
              <span class="text-yellow-500 font-bold">T</span>
              <span class="font-semibold">{{ selectedMedia.vote_average?.toFixed(1) || '4.3' }}/5</span>
              <span class="text-gray-500">{{ selectedMedia.vote_count?.toLocaleString() || '9.4K' }} ratings</span>
            </div>
            <p class="text-gray-700 mb-4">
              {{ selectedMedia.overview || 'Ethan Hunt and the IMF team race against time to find the Entity, a rogue artificial intelligence that can destroy mankind.' }}
            </p>
            <div class="flex items-center gap-4 bg-gray-100 rounded-lg p-4">
              <div class="relative">
                <img
                  :src="selectedMedia.poster_path ? `https://image.tmdb.org/t/p/w154${selectedMedia.poster_path}` : 'https://via.placeholder.com/150x225?text=No+Image'"
                  :alt="selectedMedia.title || selectedMedia.name"
                  class="w-16 aspect-[2/3] object-cover rounded"
                />
                <div class="absolute inset-0 flex items-center justify-center">
                  <div class="bg-black/60 rounded-full p-3">
                    <svg class="w-6 h-6 text-white" fill="currentColor" viewBox="0 0 24 24">
                      <polygon points="5 3 19 12 5 21 5 3"/>
                    </svg>
                  </div>
                </div>
              </div>
              <div>
                <p class="font-semibold">Watch trailer</p>
                <p class="text-gray-500 text-sm">02:01</p>
              </div>
            </div>
          </div>
          <div class="flex items-center gap-2 text-gray-500">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2"/>
              <circle cx="9" cy="7" r="4"/>
              <path d="M23 21v-2a4 4 0 00-3-3.87"/>
              <path d="M16 3.13a4 4 0 010 7.75"/>
            </svg>
            <span>{{ getRandomCount() }} added this movie</span>
          </div>
          <div class="mb-4">
            <p class="font-semibold mb-2">Cast</p>
            <p class="text-gray-500">Cast list will appear here...</p>
          </div>
        </div>
      </div>
      <button
        v-if="!watchlistMediaIds.has(selectedMedia.id)"
        @click="addToWatchlist(selectedMedia.id, selectedMedia.media_type)"
        class="fixed bottom-20 left-0 right-0 mx-6 bg-yellow-400 hover:bg-yellow-500 text-black font-bold text-xl py-4 rounded-t-xl flex items-center justify-center gap-2"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <line x1="12" y1="5" x2="12" y2="19" stroke-width="2"/>
          <line x1="5" y1="12" x2="19" y2="12" stroke-width="2"/>
        </svg>
        {{ selectedMedia.media_type === 'tv' ? 'ADD SHOW' : 'ADD MOVIE' }}
      </button>
      <button
        v-else
        class="fixed bottom-20 left-0 right-0 mx-6 bg-yellow-400 text-black font-bold text-xl py-4 rounded-t-xl flex items-center justify-center gap-2"
        disabled
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <polyline points="20 6 9 17 4 12" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        {{ selectedMedia.media_type === 'tv' ? 'SHOW ADDED' : 'MOVIE ADDED' }}
      </button>
    </div>
    <!-- Fixed Bottom Navigation -->
    <div class="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200 flex justify-around items-center py-3 z-50">
      <button
        @click="currentPage = 'home'"
        :class="currentPage === 'home' ? 'text-blue-600' : 'text-gray-400'"
        class="flex flex-col items-center gap-1 hover:text-gray-600 transition-colors"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <rect x="3" y="4" width="18" height="18" rx="2" ry="2" stroke-width="2"/>
          <line x1="16" y1="2" x2="16" y2="6" stroke-width="2"/>
          <line x1="8" y1="2" x2="8" y2="6" stroke-width="2"/>
          <line x1="3" y1="10" x2="21" y2="10" stroke-width="2"/>
        </svg>
        <span class="text-xs font-medium">الرئيسية</span>
      </button>
      <button
        @click="currentPage = 'movies'"
        :class="currentPage === 'movies' ? 'text-blue-600' : 'text-gray-400'"
        class="flex flex-col items-center gap-1 hover:text-gray-600 transition-colors"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18" stroke-width="2"/>
          <line x1="7" y1="2" x2="7" y2="22" stroke-width="2"/>
          <polygon points="17,2 17,9 12,7 7,9 7,2" fill="currentColor" stroke-width="0"/>
        </svg>
        <span class="text-xs font-medium">أفلام</span>
      </button>
      <button
        @click="currentPage = 'discover'"
        :class="currentPage === 'discover' ? 'text-blue-600' : 'text-gray-400'"
        class="flex flex-col items-center gap-1 hover:text-gray-600 transition-colors"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <circle cx="11" cy="11" r="8" stroke-width="2"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65" stroke-width="2"/>
        </svg>
        <span class="text-xs font-medium">استكشف</span>
      </button>
      <button
        @click="currentPage = 'profile'"
        :class="currentPage === 'profile' ? 'text-blue-600' : 'text-gray-400'"
        class="flex flex-col items-center gap-1 hover:text-gray-600 transition-colors"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" stroke-width="2"/>
          <circle cx="12" cy="7" r="4" stroke-width="2"/>
        </svg>
        <span class="text-xs font-medium">الملف الشخصي</span>
      </button>
    </div>
  </div>
</template>
