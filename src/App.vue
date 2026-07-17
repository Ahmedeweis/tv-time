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
const loading = ref(false);
const showTrailerModal = ref(false);
const trailerKey = ref<string | null>(null);
const tvSeasons = ref<any[]>([]);
const watchedEpisodes = ref<Set<string>>(new Set());
const similarMedia = ref<any[]>([]);
const similarScrollRef = ref<HTMLElement | null>(null);
const mediaHistory = ref<any[]>([]); // Track history for back button navigation
const activeDetailsTab = ref<'about' | 'more' | 'episodes'>('about');
const activeShowsTab = ref<'watchlist' | 'upcoming'>('watchlist');
const trailerVideo = ref<any>(null);
const expandedSeason = ref<number | null>(null);
const seasonEpisodes = ref<any[]>([]);
const watchlistMediaIds = computed(() => {
  const ids = new Set<number>();
  releasedWatchlist.value.forEach(([media]) => ids.add(media.media_id));
  upcomingWatchlist.value.forEach(([media]) => ids.add(media.media_id));
  return ids;
});
const watchedReleased = computed(() => releasedWatchlist.value.filter(m => m[1].status === 'watched' && m[0].media_type !== 'tv'));
const unwatchedReleased = computed(() => releasedWatchlist.value.filter(m => m[1].status === 'to_watch' && m[0].media_type !== 'tv'));

const showsWatchNext = computed(() => {
  return releasedWatchlist.value.filter(m => m[0].media_type === 'tv' && (m[1].watched_episode_count || 0) > 0);
});
const showsHaventStarted = computed(() => {
  return releasedWatchlist.value.filter(m => m[0].media_type === 'tv' && (m[1].watched_episode_count || 0) === 0);
});
const showsUpcoming = computed(() => {
  return upcomingWatchlist.value.filter(m => m[0].media_type === 'tv');
});
const isWatched = (mediaId: number) => {
  const entry = [...releasedWatchlist.value, ...upcomingWatchlist.value].find(([m]) => m.media_id === mediaId);
  return entry ? entry[1].status === 'watched' : false;
};
const getRandomCount = () => {
  const min = 79;
  const max = 700000;
  const num = Math.floor(Math.random() * (max - min + 1)) + min;
  if (num >= 1000) {
    return (num / 1000).toFixed(1).replace(/\.0$/, '') + 'k';
  }
  return num.toString();
};
const fetchTrailer = async (mediaId: number, mediaType: string) => {
  try {
    const videos = await invoke('get_tmdb_videos', { mediaId, mediaType });
    const results = (videos as any).results || [];
    // Find a trailer (prefer official trailers first)
    const trailer = results.find((v: any) => v.type === 'Trailer' && v.site === 'YouTube') ||
                   results.find((v: any) => v.site === 'YouTube');
    if (trailer) {
      trailerKey.value = trailer.key;
    } else {
      trailerKey.value = null;
    }
  } catch (error) {
    console.error('Failed to fetch trailer:', error);
    trailerKey.value = null;
  }
};
const fetchSimilarMedia = async (mediaId: number, mediaType: string) => {
  try {
    let combined = [];
    // Try to get similar media
    try {
      const similar = await invoke('get_tmdb_similar', { mediaId, mediaType });
      const similarResults = (similar as any).results || [];
      combined = [...similarResults];
    } catch (e) {
      console.error('Failed to fetch similar media:', e);
    }
    // If not enough, add recommendations
    if (combined.length < 6) {
      try {
        const recommendations = await invoke('get_tmdb_recommendations', { mediaId, mediaType });
        const recResults = (recommendations as any).results || [];
        // Add recommendations that aren't already in the list
        const existingIds = new Set(combined.map((m: any) => m.id));
        const newRecs = recResults.filter((m: any) => !existingIds.has(m.id));
        combined = [...combined, ...newRecs];
      } catch (e) {
        console.error('Failed to fetch recommendations:', e);
      }
    }
    // Filter out current media and set media_type
    similarMedia.value = combined
      .filter((m: any) => m.id !== mediaId)
      .slice(0, 12) // Show max 12 items
      .map((m: any) => ({
        ...m,
        media_type: mediaType
      }));
  } catch (error) {
    console.error('Failed to fetch similar media:', error);
    similarMedia.value = [];
  }
};
const scrollLeft = () => {
  if (similarScrollRef.value) {
    similarScrollRef.value.scrollBy({ left: -300, behavior: 'smooth' });
  }
};
const scrollRight = () => {
  if (similarScrollRef.value) {
    similarScrollRef.value.scrollBy({ left: 300, behavior: 'smooth' });
  }
};
const goBack = () => {
  if (mediaHistory.value.length > 0) {
    const previousMedia = mediaHistory.value.pop();
    selectedMedia.value = previousMedia;
    activeDetailsTab.value = 'about';
    // Fetch trailer and similar media for previous media
    fetchTrailer(previousMedia.id, previousMedia.media_type);
    fetchSimilarMedia(previousMedia.id, previousMedia.media_type);
  } else {
    currentPage.value = 'home';
    mediaHistory.value = [];
  }
};
const openTrailer = () => {
  if (trailerKey) {
    showTrailerModal.value = true;
  } else {
    // If no trailer found, open YouTube search
    const searchQuery = encodeURIComponent(`${selectedMedia.value.title || selectedMedia.value.name} trailer`);
    window.open(`https://www.youtube.com/results?search_query=${searchQuery}`, '_blank');
  }
};
const openMediaDetails = async (item: any, mediaType?: string) => {
  // If item has synopsis_cached_json_data (from watchlist), parse it to get full TMDB data
  let fullMediaData = item;
  if (item.synopsis_cached_json_data) {
    try {
      fullMediaData = JSON.parse(item.synopsis_cached_json_data);
    } catch (e) {
      console.error('Failed to parse cached media data:', e);
    }
  }
  const newMedia = {
    ...fullMediaData,
    id: item.id || item.media_id, // Normalize ID
    media_type: mediaType || item.media_type || (item.title ? 'movie' : 'tv')
  };
  // Push current media to history if we're already in details page, otherwise clear history
  if (currentPage.value === 'details' && selectedMedia.value) {
    mediaHistory.value.push(selectedMedia.value);
  } else {
    mediaHistory.value = [];
  }
  selectedMedia.value = newMedia;
  currentPage.value = 'details';
  activeDetailsTab.value = 'about'; // Reset to About tab when opening new media
  // Fetch trailer
  const mediaId = selectedMedia.value.id;
  const type = selectedMedia.value.media_type;
  fetchTrailer(mediaId, type);
  // Fetch similar media
  fetchSimilarMedia(mediaId, type);
  // Fetch TV seasons if it's a TV show
  if (type === 'tv') {
    fetchTVSeasons(mediaId);
  }
};

const fetchTVSeasons = async (mediaId: number) => {
  try {
    tvSeasons.value = [];
    watchedEpisodes.value.clear();
    
    // Get seasons
    const data = await invoke('get_tv_seasons', { mediaId });
    if (data.seasons) {
      const seasons = data.seasons;
      seasons.sort((a: any, b: any) => {
        if (a.season_number === 0) return 1;
        if (b.season_number === 0) return -1;
        return a.season_number - b.season_number;
      });
      tvSeasons.value = seasons;
    }
    
    // Get watched episodes
    const watched: any[] = await invoke('get_watched_episodes', { mediaId });
    const newWatched = new Set<string>();
    for (const ep of watched) {
      newWatched.add(`${ep.season_number}-${ep.episode_number}`);
    }
    watchedEpisodes.value = newWatched;
  } catch (e) {
    console.error('Error fetching TV seasons:', e);
  }
};

const toggleEpisode = async (mediaId: number, seasonNumber: number, episodeNumber: number) => {
  try {
    await invoke('toggle_episode_watched', { mediaId, seasonNumber, episodeNumber });
    const key = `${seasonNumber}-${episodeNumber}`;
    if (watchedEpisodes.value.has(key)) {
      watchedEpisodes.value.delete(key);
    } else {
      watchedEpisodes.value.add(key);
    }
    // Reload dashboard to update counts
    await loadDashboard();
  } catch (e) {
    console.error('Error toggling episode:', e);
  }
};

const toggleSeason = async (seasonNumber: number, mediaId: number) => {
  if (expandedSeason.value === seasonNumber) {
    expandedSeason.value = null;
    seasonEpisodes.value = [];
  } else {
    expandedSeason.value = seasonNumber;
    seasonEpisodes.value = [];
    try {
      const data: any = await invoke('get_tv_season_episodes', { mediaId, seasonNumber });
      if (data.episodes) {
        seasonEpisodes.value = data.episodes;
      }
    } catch (e) {
      console.error(e);
    }
  }
};

const markSeasonAsWatched = async (mediaId: number, seasonNumber: number) => {
  try {
    const data: any = await invoke('get_tv_season_episodes', { mediaId, seasonNumber });
    if (data.episodes) {
      for (const ep of data.episodes) {
        const key = `${seasonNumber}-${ep.episode_number}`;
        if (!watchedEpisodes.value.has(key)) {
          await invoke('toggle_episode_watched', { mediaId, seasonNumber, episodeNumber: ep.episode_number });
          watchedEpisodes.value.add(key);
        }
      }
      await loadDashboard();
    }
  } catch (e) {
    console.error('Error marking season as watched:', e);
  }
};

const unmarkSeasonAsWatched = async (mediaId: number, seasonNumber: number) => {
  try {
    const data: any = await invoke('get_tv_season_episodes', { mediaId, seasonNumber });
    if (data.episodes) {
      for (const ep of data.episodes) {
        const key = `${seasonNumber}-${ep.episode_number}`;
        if (watchedEpisodes.value.has(key)) {
          await invoke('toggle_episode_watched', { mediaId, seasonNumber, episodeNumber: ep.episode_number });
          watchedEpisodes.value.delete(key);
        }
      }
      await loadDashboard();
    }
  } catch (e) {
    console.error('Error unmarking season as watched:', e);
  }
};

const showUnwatchConfirm = ref(false);
const seasonToUnwatch = ref<{ mediaId: number; seasonNumber: number } | null>(null);

const handleSeasonCheckClick = (mediaId: number, seasonNumber: number) => {
  const watchedCount = Array.from(watchedEpisodes.value).filter(k => k.startsWith(seasonNumber + '-')).length;
  const totalCount = tvSeasons.value.find((s: any) => s.season_number === seasonNumber)?.episode_count || 0;
  
  if (watchedCount === totalCount && totalCount > 0) {
    // All episodes watched - show confirmation to unwatch
    seasonToUnwatch.value = { mediaId, seasonNumber };
    showUnwatchConfirm.value = true;
  } else {
    // Not all watched - mark all as watched
    markSeasonAsWatched(mediaId, seasonNumber);
  }
};

const confirmUnwatchSeason = () => {
  if (seasonToUnwatch.value) {
    unmarkSeasonAsWatched(seasonToUnwatch.value.mediaId, seasonToUnwatch.value.seasonNumber);
    showUnwatchConfirm.value = false;
    seasonToUnwatch.value = null;
  }
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
const removeFromWatchlist = async (mediaId: number, mediaType: string) => {
  try {
    await invoke('remove_from_watchlist', { mediaId });
    if (tmdbSessionId.value) {
      await toggleTmdbWatchlist(mediaId, mediaType, false);
    }
    await loadDashboard();
  } catch (error) {
    console.error('Remove from watchlist error:', error);
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
const markAsToWatch = async (mediaId: number) => {
  try {
    await invoke('mark_as_to_watch', { mediaId });
    await loadDashboard();
  } catch (error) {
    console.error('Mark as to watch error:', error);
  }
};
const toggleWatchedStatus = async (mediaId: number) => {
  if (isWatched(mediaId)) {
    await markAsToWatch(mediaId);
  } else {
    await markAsWatched(mediaId);
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

const getDaysUntilEpisode = (airDate: string | null) => {
  if (!airDate) return null;
  const air = new Date(airDate);
  const today = new Date();
  const diff = air.getTime() - today.getTime();
  const days = Math.ceil(diff / (1000 * 3600 * 24));
  return days;
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
      <div class="flex justify-around items-center mb-6 border-b-2 border-gray-200 pt-5">
        <button
          @click="activeTab = 'upcoming'"
          class="pb-2 px-6 text-xl font-bold transition-colors"
          :class="activeTab === 'upcoming' ? 'text-yellow-400 border-b-4 border-yellow-400' : 'text-gray-400'"
        >
          Upcoming
        </button>
        <button
          @click="activeTab = 'unwatched'"
          class="pb-2 px-6 text-xl font-bold transition-colors"
          :class="activeTab === 'unwatched' ? 'text-yellow-400 border-b-4 border-yellow-400' : 'text-gray-400'"
        >
          Watchlist
        </button>
      </div>
      <!-- Upcoming Tab Content -->
      <div v-if="activeTab === 'upcoming'">
        <div v-if="upcomingWatchlist.length > 0" class="grid grid-cols-6 gap-2">
          <div
            v-for="[media, _] in upcomingWatchlist"
            :key="media.media_id"
            class="flex flex-col items-center cursor-pointer hover:opacity-80 relative"
            @click="openMediaDetails(media)"
          >
            <div class="relative w-full">
              <img
                :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="media.title"
                class="w-full aspect-[2/3] object-cover rounded-lg"
              />
              <div class="absolute bottom-2 left-2 bg-black/70 text-white px-2 py-1 rounded-md">
                <span class="text-lg font-bold">{{ getDaysUntilRelease(media.release_date) }}</span>
              <br>
                <span class="text-lg ">days</span>
              </div>
            </div>
          </div>
        </div>
        <p v-else class="text-gray-400 text-center">No upcoming items yet.</p>
      </div>
      <!-- Unwatched Tab Content -->
      <div v-else-if="activeTab === 'unwatched'">
        <!-- Unwatched section -->
        <div v-if="unwatchedReleased.length > 0" class="mb-8">
          <div class="grid grid-cols-6 gap-2">
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
            </div>
          </div>
        </div>
        <p v-else-if="watchedReleased.length === 0" class="text-gray-400 text-center mb-8">No unwatched items yet.</p>
        <!-- Watched section -->
        <div v-if="watchedReleased.length > 0">
          <h2 class="text-xl font-bold mb-4 border-b-2 border-gray-200 pb-2 text-center">Watched</h2>
          <div class="grid grid-cols-6 gap-2">
            <div
              v-for="[media, watchlist] in watchedReleased"
              :key="media.media_id"
              class="flex flex-col items-center cursor-pointer  relative"
              @click="openMediaDetails(media)"
            >
              <img
                :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="media.title"
                class="w-full aspect-[2/3] object-cover rounded-lg "
              />
            </div>
          </div>
        </div>
      </div>
    </div>
    <!-- Shows Page -->
    <div v-else-if="currentPage === 'shows'">
      <div class="flex border-b-2 border-gray-200 mb-6 mt-4">
        <button
          @click="activeShowsTab = 'watchlist'"
          class="pb-2 px-6 text-center font-bold w-1/2 transition-colors"
          :class="activeShowsTab === 'watchlist' ? 'text-black border-b-4 border-black' : 'text-gray-400'"
        >
          WATCH LIST
        </button>
        <button
          @click="activeShowsTab = 'upcoming'"
          class="pb-2 px-6 text-center font-bold w-1/2 transition-colors"
          :class="activeShowsTab === 'upcoming' ? 'text-black border-b-4 border-black' : 'text-gray-400'"
        >
          UPCOMING
        </button>
      </div>
      
      <div v-if="activeShowsTab === 'watchlist'">
        <!-- Watch Next section -->
        <div class="mb-8">
          <h2 class="text-center text-sm font-bold bg-gray-400 text-white rounded-full px-4 py-1 inline-block mb-4 mx-auto block w-max">WATCH NEXT</h2>
          <div v-if="showsWatchNext.length > 0" class="grid grid-cols-6 gap-2">
            <div
              v-for="[media, watchlist] in showsWatchNext"
              :key="media.media_id"
              class="flex flex-col items-center cursor-pointer relative"
              @click="openMediaDetails(media, 'tv')"
            >
              <img
                :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="media.title"
                class="w-full aspect-[2/3] object-cover rounded-lg"
              />
              <div class="w-full mt-2 bg-gray-200 h-1.5 rounded-full overflow-hidden">
                 <div class="bg-yellow-400 h-full" style="width: 50%"></div>
              </div>
            </div>
          </div>
          <p v-else class="text-gray-400 text-center mb-8">No shows in progress.</p>
        </div>
        
        <!-- Haven't Started section -->
        <div>
          <h2 class="text-center text-sm font-bold bg-gray-400 text-white rounded-full px-4 py-1 inline-block mb-4 mx-auto block w-max">HAVEN'T STARTED</h2>
          <div v-if="showsHaventStarted.length > 0" class="grid grid-cols-6 gap-2">
            <div
              v-for="[media, watchlist] in showsHaventStarted"
              :key="media.media_id"
              class="flex flex-col items-center cursor-pointer relative overflow-hidden aspect-[2/3] group"
              @click="openMediaDetails(media, 'tv')"
            >
              <img
                :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="media.title"
                class="absolute inset-0 w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
              />
            </div>
          </div>
          <p v-else class="text-gray-400 text-center mb-8">No shows to start.</p>
        </div>
      </div>
      
      <div v-else-if="activeShowsTab === 'upcoming'">
        <div v-if="showsUpcoming.length > 0" class="grid grid-cols-6 gap-2">
          <div
            v-for="[media, _] in showsUpcoming"
            :key="media.media_id"
            class="flex flex-col items-center cursor-pointer relative"
            @click="openMediaDetails(media, 'tv')"
          >
            <img
              :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
              :alt="media.title"
              class="w-full aspect-[2/3] object-cover rounded-lg"
            />
          </div>
        </div>
        <p v-else class="text-gray-400 text-center mb-8">No upcoming shows.</p>
      </div>
    </div>
    <!-- Discover Page (new) -->
    <div v-else-if="currentPage === 'discover'">
      <div class="mb-6">
        <div class="flex gap-4 items-center pt-5">
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
            class="w-12 h-12 flex items-center justify-center bg-white border-2 border-gray-400 rounded-lg hover:bg-gray-100"
          >
            <svg class="w-6 h-6 text-gray-700" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <line x1="12" y1="5" x2="12" y2="19" stroke-width="2"/>
              <line x1="5" y1="12" x2="19" y2="12" stroke-width="2"/>
            </svg>
          </button>
          <div
            v-else
            class="w-12 h-12 flex items-center justify-center bg-yellow-400 rounded-lg text-black"
            title="Already in watchlist"
          >
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <polyline points="20 6 9 17 4 12" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></polyline>
            </svg>
          </div>
        </div>
      </div>
    </div>
    <!-- Media Details Page -->
    <div v-else-if="currentPage === 'details' && selectedMedia" class="pb-24 -mx-6">
      <div class="relative">
        <img
          :src="selectedMedia.backdrop_path ? `https://image.tmdb.org/t/p/w1280${selectedMedia.backdrop_path}` : (selectedMedia.poster_path ? `https://image.tmdb.org/t/p/w780${selectedMedia.poster_path}` : 'https://via.placeholder.com/1280x720?text=No+Image')"
          :alt="selectedMedia.title || selectedMedia.name"
          class="w-full max-h-[35vh] aspect-video object-cover object-center"
        />
        <div class="absolute inset-0 bg-gradient-to-t from-black via-transparent to-transparent" />
        <button
          @click="goBack"
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
      <div class="px-6 pt-6 space-y-4 bg-[#F6F6F6]">
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
            @click="toggleWatchedStatus(selectedMedia.id)"
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
        <div class="flex border-b border-gray-200 w-full">
          <button
            @click="activeDetailsTab = 'about'"
            class="pb-2 px-6 text-center font-semibold w-1/2 transition-colors"
            :class="activeDetailsTab === 'about' ? 'border-b-2 border-black' : 'text-gray-500'"
          >
            ABOUT
          </button>
          <button
            v-if="selectedMedia.media_type === 'tv' || selectedMedia.name"
            @click="activeDetailsTab = 'episodes'"
            class="pb-2 px-6 text-center font-semibold w-1/2 transition-colors"
            :class="activeDetailsTab === 'episodes' ? 'border-b-2 border-black' : 'text-gray-500'"
          >
            EPISODES
          </button>
          <button
            v-else
            @click="activeDetailsTab = 'more'"
            class="pb-2 px-6 text-center font-semibold w-1/2 transition-colors"
            :class="activeDetailsTab === 'more' ? 'border-b-2 border-black' : 'text-gray-500'"
          >
            MORE
          </button>
        </div>
        <div v-if="activeDetailsTab === 'about'" class="space-y-4">
          <div class="flex justify-between items-center ">
            <p class="text-gray-600">Where to watch</p>
          </div>
          <p class="text-gray-500 pb-4 border-b border-gray-200">Not available</p>
          <div>
            <p class="text-gray-600 mb-2">Movie info</p>
            <div class="flex items-center gap-2 mb-2">
              <img :src="logoUrl" alt="App icon" class="w-5 h-5">
              <div class="flex items-center gap-0.5">
                <svg class="w-4 h-4 text-yellow-500" fill="currentColor" viewBox="0 0 24 24">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
                <svg class="w-4 h-4 text-yellow-500" fill="currentColor" viewBox="0 0 24 24">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
                <svg class="w-4 h-4 text-yellow-500" fill="currentColor" viewBox="0 0 24 24">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
                <svg class="w-4 h-4 text-yellow-500" fill="currentColor" viewBox="0 0 24 24">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
                <svg class="w-4 h-4 text-yellow-500" fill="currentColor" viewBox="0 0 24 24">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
              </div>
              <span class="font-semibold">{{ (selectedMedia.vote_average / 2)?.toFixed(1) || '4.3' }}/5</span>
              <span class="text-gray-500">{{ selectedMedia.vote_count?.toLocaleString() || '9.4K' }} ratings</span>
            </div>
            <p class="text-gray-700 mb-4 border-b border-gray-200 pb-2">
              {{ selectedMedia.overview || 'Ethan Hunt and the IMF team race against time to find the Entity, a rogue artificial intelligence that can destroy mankind.' }}
            </p>
            <div
              @click="openTrailer"
              class="flex items-center gap-4 rounded-lg p-4 mb-4 border-b border-gray-200 pb-2  cursor-pointer transition-colors"
            >
              <div class="relative">
                <img
                  :src="selectedMedia.backdrop_path ? `https://image.tmdb.org/t/p/w300${selectedMedia.backdrop_path}` : (selectedMedia.poster_path ? `https://image.tmdb.org/t/p/w300${selectedMedia.poster_path}` : 'https://via.placeholder.com/300x170?text=No+Image')"
                  :alt="selectedMedia.title || selectedMedia.name"
                  class="w-32 aspect-[16/9] object-cover rounded"
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
                <p class="text-gray-500 text-lg">02:01</p>
              </div>
            </div>
            <!-- Trailer Modal -->
            <div v-if="showTrailerModal" class="fixed inset-0  bg-black/80 z-50 flex items-center justify-center p-4">
              <div class="relative w-full max-w-4xl aspect-video">
                <button
                  @click="showTrailerModal = false"
                  class="absolute -top-12 right-0 text-white text-3xl hover:text-gray-300"
                >
                  &times;
                </button>
                <iframe
                  :src="`https://www.youtube.com/embed/${trailerKey}?autoplay=1`"
                  :title="`${selectedMedia.title || selectedMedia.name} Trailer`"
                  class="w-full h-full rounded-lg"
                  frameborder="0"
                  allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                  allowfullscreen
                ></iframe>
              </div>
            </div>
          </div>
          <div class="flex items-center gap-2 text-gray-500 mb-4 border-b border-gray-200 pb-2">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2"/>
              <circle cx="9" cy="7" r="4"/>
              <path d="M23 21v-2a4 4 0 00-3-3.87"/>
              <path d="M16 3.13a4 4 0 010 7.75"/>
            </svg>
            <span>{{ getRandomCount() }} added this movie</span>
          </div>
          <div class="mb-4">
            <p class="font-semibold mb-3 text-lg">Similar</p>
            <div class="relative">
              <button
                @click="scrollLeft"
                class="absolute left-0 top-1/2 -translate-y-1/2 z-10 bg-white/80 hover:bg-white rounded-full p-1 shadow-md"
              >
                <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                </svg>
              </button>
              <div
                ref="similarScrollRef"
                class="flex gap-3 overflow-x-auto pb-2 scrollbar-hide"
              >
                <div
                  v-for="media in similarMedia"
                  :key="media.id"
                  class="cursor-pointer hover:opacity-80 flex-shrink-0 w-1/6 relative"
                >
                  <img
                    :src="media.backdrop_path ? `https://image.tmdb.org/t/p/w300${media.backdrop_path}` : (media.poster_path ? `https://image.tmdb.org/t/p/w300${media.poster_path}` : 'https://via.placeholder.com/300x170?text=No+Image')"
                    :alt="media.title || media.name"
                    class="w-full aspect-[16/9] object-cover rounded-lg"
                    @click="openMediaDetails(media, media.media_type)"
                  />
                  <button
                    @click.stop="addToWatchlist(media.id, media.media_type)"
                    class="absolute top-2 right-2 w-6 h-6 flex items-center justify-center rounded transition-colors"
                    :class="watchlistMediaIds.has(media.id) ? 'bg-yellow-400' : 'border-2 border-yellow-400'"
                  >
                    <svg
                      v-if="watchlistMediaIds.has(media.id)"
                      class="w-4 h-4 text-black"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                    </svg>
                    <svg
                      v-else
                      class="w-4 h-4 text-yellow-400"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M12 4v16m8-8H4" />
                    </svg>
                  </button>
                  <p class="text-sm font-medium mt-1 truncate">{{ media.title || media.name }}</p>
                </div>
              </div>
              <button
                @click="scrollRight"
                class="absolute right-0 top-1/2 -translate-y-1/2 z-10 bg-white/80 hover:bg-white rounded-full p-1 shadow-md"
              >
                <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                </svg>
              </button>
            </div>
            <p v-if="similarMedia.length === 0" class="text-gray-500">No similar media found.</p>
          </div>
        </div>
        <div v-else-if="activeDetailsTab === 'more'" class="space-y-4">
          <div class="grid grid-cols-2 gap-4">
            <div class="bg-gray-50 p-3 rounded-lg">
              <p class="text-gray-500 text-sm">Status</p>
              <p class="font-semibold">{{ selectedMedia.status || 'Released' }}</p>
            </div>
            <div class="bg-gray-50 p-3 rounded-lg">
              <p class="text-gray-500 text-sm">Release Date</p>
              <p class="font-semibold">{{ selectedMedia.release_date || selectedMedia.first_air_date || 'N/A' }}</p>
            </div>
            <div class="bg-gray-50 p-3 rounded-lg">
              <p class="text-gray-500 text-sm">Language</p>
              <p class="font-semibold">{{ selectedMedia.original_language?.toUpperCase() || 'English' }}</p>
            </div>
            <div class="bg-gray-50 p-3 rounded-lg">
              <p class="text-gray-500 text-sm">Popularity</p>
              <p class="font-semibold">{{ selectedMedia.popularity?.toFixed(1) || '0' }}</p>
            </div>
            <div class="bg-gray-50 p-3 rounded-lg col-span-2">
              <p class="text-gray-500 text-sm">Genres</p>
              <div class="flex flex-wrap gap-2 mt-1">
                <span v-for="genre in (selectedMedia.genres || [])" :key="genre.id" class="bg-yellow-100 text-yellow-800 px-3 py-1 rounded-full text-sm">
                  {{ genre.name }}
                </span>
              </div>
            </div>
            <div v-if="selectedMedia.runtime" class="bg-gray-50 p-3 rounded-lg">
              <p class="text-gray-500 text-sm">Runtime</p>
              <p class="font-semibold">{{ Math.floor(selectedMedia.runtime / 60) }}h {{ selectedMedia.runtime % 60 }}m</p>
            </div>
            <div v-if="selectedMedia.number_of_seasons" class="bg-gray-50 p-3 rounded-lg">
              <p class="text-gray-500 text-sm">Seasons</p>
              <p class="font-semibold">{{ selectedMedia.number_of_seasons }}</p>
            </div>
            <div v-if="selectedMedia.number_of_episodes" class="bg-gray-50 p-3 rounded-lg">
              <p class="text-gray-500 text-sm">Episodes</p>
              <p class="font-semibold">{{ selectedMedia.number_of_episodes }}</p>
            </div>
          </div>
          <div v-if="selectedMedia.tagline" class="border-t border-gray-200 pt-4">
            <p class="text-gray-500 text-sm mb-2">Tagline</p>
            <p class="italic text-gray-700">"{{ selectedMedia.tagline }}"</p>
          </div>
        </div>
        <div v-else-if="activeDetailsTab === 'episodes'" class="space-y-4" style="background-color: #F6F6F6; padding: 16px; border-radius: 8px;">
          <div v-if="tvSeasons.length === 0" class="text-gray-500 text-center py-4">No seasons found.</div>
          <div v-else class="space-y-2">
            <div v-for="season in tvSeasons" :key="season.id" class="border border-gray-200 rounded-lg overflow-hidden">
              <button 
                @click="toggleSeason(season.season_number, selectedMedia.id)"
                class="w-full flex justify-between items-center p-4 bg-white hover:bg-gray-100 transition-colors"
              >
                <div class="flex items-center gap-3">
                  <span class="font-semibold text-lg">{{ season.name }}</span>
                  <svg :class="['w-5 h-5 transition-transform', expandedSeason === season.season_number ? 'rotate-180' : '']" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                  </svg>
                </div>
                <div class="flex items-center gap-3">
                  <span class="text-gray-500 text-sm">{{ Array.from(watchedEpisodes).filter(k => k.startsWith(season.season_number + '-')).length }}/{{ season.episode_count }}</span>
                  <button
                    @click.stop="handleSeasonCheckClick(selectedMedia.id, season.season_number)"
                    :class="['w-8 h-8 rounded-full flex items-center justify-center border-2 shrink-0', Array.from(watchedEpisodes).filter(k => k.startsWith(season.season_number + '-')).length === season.episode_count ? 'bg-green-500 border-green-500 text-white' : 'border-gray-300 text-gray-300 hover:border-gray-400']"
                    title="Mark all episodes as watched"
                  >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <polyline points="20 6 9 17 4 12" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  </button>
                </div>
              </button>
              
              <div v-if="expandedSeason === season.season_number" class="bg-white p-2 border-t border-gray-200">
                <div v-if="seasonEpisodes.length === 0" class="text-center text-gray-400 py-4">Loading episodes...</div>
                <div v-else class="space-y-2">
                  <div 
                    v-for="ep in seasonEpisodes" 
                    :key="ep.id"
                    :class="['flex items-center gap-3 p-2 rounded-lg', getDaysUntilEpisode(ep.air_date) !== null && getDaysUntilEpisode(ep.air_date) > 0 ? 'bg-yellow-50' : 'hover:bg-gray-50']"
                  >
                    <img 
                      :src="ep.still_path ? `https://image.tmdb.org/t/p/w185${ep.still_path}` : 'https://via.placeholder.com/185x104?text=No+Image'" 
                      :alt="ep.name"
                      class="w-24 aspect-video object-cover rounded"
                    />
                    <div class="flex-1">
                      <p class="font-semibold text-sm text-gray-700">S{{ season.season_number.toString().padStart(2, '0') }} | E{{ ep.episode_number.toString().padStart(2, '0') }}</p>
                      <p class="text-sm text-gray-500 line-clamp-1">{{ ep.name }}</p>
                    </div>
                    <div v-if="getDaysUntilEpisode(ep.air_date) !== null && getDaysUntilEpisode(ep.air_date) > 0" class="text-lg font-bold text-black">
                      {{ getDaysUntilEpisode(ep.air_date) }} Day
                    </div>
                    <button 
                      v-else
                      @click="toggleEpisode(selectedMedia.id, season.season_number, ep.episode_number)"
                      :class="['w-8 h-8 rounded-full flex items-center justify-center border-2 shrink-0', watchedEpisodes.has(season.season_number + '-' + ep.episode_number) ? 'bg-green-500 border-green-500 text-white' : 'border-gray-300 text-gray-300 hover:border-gray-400']"
                    >
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <polyline points="20 6 9 17 4 12" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
        <!-- Unwatch Season Confirmation Modal -->
        <div v-if="showUnwatchConfirm" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
          <div class="bg-white rounded-lg p-6 max-w-sm w-full">
            <h3 class="text-lg font-bold mb-4">Unwatch Season?</h3>
            <p class="text-gray-600 mb-6">Are you sure you want to mark all episodes in this season as not watched?</p>
            <div class="flex gap-3">
              <button
                @click="showUnwatchConfirm = false; seasonToUnwatch = null"
                class="flex-1 bg-gray-200 hover:bg-gray-300 text-black font-semibold py-3 rounded-lg"
              >
                Cancel
              </button>
              <button
                @click="confirmUnwatchSeason"
                class="flex-1 bg-red-500 hover:bg-red-600 text-white font-semibold py-3 rounded-lg"
              >
                Yes, Unwatch
              </button>
            </div>
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
        @click="removeFromWatchlist(selectedMedia.id, selectedMedia.media_type)"
        class="fixed bottom-20 left-0 right-0 mx-6 bg-red-500 hover:bg-red-600 text-white font-bold text-xl py-4 rounded-t-xl flex items-center justify-center gap-2"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <line x1="18" y1="6" x2="6" y2="18" stroke-width="2"/>
          <line x1="6" y1="6" x2="18" y2="18" stroke-width="2"/>
        </svg>
        {{ selectedMedia.media_type === 'tv' ? 'REMOVE SHOW' : 'REMOVE MOVIE' }}
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
        <span class="text-xs font-medium">Movies</span>
      </button>
      <button
        @click="currentPage = 'shows'"
        :class="currentPage === 'shows' ? 'text-blue-600' : 'text-gray-400'"
        class="flex flex-col items-center gap-1 hover:text-gray-600 transition-colors"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18" stroke-width="2"/>
          <line x1="7" y1="2" x2="7" y2="22" stroke-width="2"/>
          <polygon points="17,2 17,9 12,7 7,9 7,2" fill="currentColor" stroke-width="0"/>
        </svg>
        <span class="text-xs font-medium">Shows</span>
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
        <span class="text-xs font-medium">Discover</span>
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
        <span class="text-xs font-medium">Profile</span>
      </button>
    </div>
  </div>
</template>
<style scoped>
.scrollbar-hide::-webkit-scrollbar {
  display: none;
}
.scrollbar-hide {
  -ms-overflow-style: none;
  scrollbar-width: none;
}
</style>
