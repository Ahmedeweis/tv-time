<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useRoute } from 'vue-router';
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
const route = useRoute();
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
  watched_episode_count?: number;
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
const upcomingEpisodes = ref<any[]>([]);
const nextEpisodesToWatch = ref<any[]>([]);
const favoriteMediaIds = ref<Set<number>>(new Set());
const favoriteItems = ref<FavoriteItem[]>([]);
interface FavoriteItem {
  media_id: number;
  title: string;
  poster_path: string | null;
  media_type: string;
  release_date: string | null;
  added_at: string;
}
// Lists state
interface UserList {
  list_id: number;
  name: string;
  created_at: string;
  updated_at: string;
  item_count: number;
}
interface ListItem {
  media_id: number;
  title: string;
  poster_path: string | null;
  media_type: string;
  release_date: string | null;
  added_at: string;
}
const userLists = ref<UserList[]>([]);
const selectedList = ref<UserList | null>(null);
const selectedListItems = ref<ListItem[]>([]);
const listPreviewItems = ref<Record<number, ListItem[]>>({});
const showCreateListModal = ref(false);
const showRenameListModal = ref(false);
const showDeleteListConfirm = ref(false);
const showAddToListModal = ref(false);
const newListName = ref('');
const renameListName = ref('');
const listToRename = ref<UserList | null>(null);
const listToDelete = ref<UserList | null>(null);
const currentListPage = ref<'lists' | 'view'>('lists');
const favoriteShows = computed(() => favoriteItems.value.filter(item => item.media_type === 'tv'));
const favoriteMovies = computed(() => favoriteItems.value.filter(item => item.media_type === 'movie'));
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
const profileWatchedMovies = computed(() => watchedReleased.value);
const profileCompletedShows = computed(() => {
  const nextIds = new Set(nextEpisodesToWatch.value.map((ep: any) => ep.media_id));
  return releasedWatchlist.value.filter(([media, wl]) => {
    if (media.media_type !== 'tv') return false;
    const watchedCount = wl.watched_episode_count ?? 0;
    if (watchedCount <= 0) return false;
    // No next episode left = finished the show
    if (!nextIds.has(media.media_id)) return true;
    try {
      const data = JSON.parse(media.synopsis_cached_json_data || '{}');
      const total = data.number_of_episodes;
      if (typeof total === 'number' && total > 0) {
        return watchedCount >= total;
      }
    } catch {
      // ignore parse errors
    }
    return false;
  });
});
const profileEpisodesWatched = computed(() =>
  releasedWatchlist.value.reduce((sum, [, wl]) => sum + (wl.watched_episode_count ?? 0), 0)
);
interface UserProfileData {
  name: string;
  avatar_data_url: string | null;
  cover_data_url: string | null;
}
const userProfile = ref<UserProfileData>({
  name: '',
  avatar_data_url: null,
  cover_data_url: null,
});
const isEditingProfileName = ref(false);
const profileNameDraft = ref('');
const avatarFileInput = ref<HTMLInputElement | null>(null);
const coverFileInput = ref<HTMLInputElement | null>(null);
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
  // Ensure poster_path is preserved from the original item if not in fullMediaData
  if (item.poster_path && !fullMediaData.poster_path) {
    fullMediaData.poster_path = item.poster_path;
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
    if (data) {
      // Update selectedMedia with full TMDB data to get backdrop, poster, overview, etc.
      if (selectedMedia.value && selectedMedia.value.id === mediaId) {
        selectedMedia.value = {
          ...selectedMedia.value,
          ...data,
          media_type: 'tv'
        };
      }
    }
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
    await loadNextEpisodesToWatch();
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
      await loadNextEpisodesToWatch();
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
      await loadNextEpisodesToWatch();
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
const loadUpcomingEpisodes = async () => {
  try {
    const result = await invoke('get_upcoming_episodes');
    upcomingEpisodes.value = result.upcoming_episodes || [];
  } catch (error) {
    console.error('Load upcoming episodes error:', error);
  }
};
const loadNextEpisodesToWatch = async () => {
  try {
    const result = await invoke('get_next_episodes_to_watch');
    nextEpisodesToWatch.value = result.next_episodes || [];
  } catch (error) {
    console.error('Load next episodes to watch error:', error);
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
const toggleFavorite = async (mediaId: number, mediaType: string) => {
  try {
    const isNowFavorited = await invoke('toggle_favorite', { mediaId, mediaType }) as boolean;
    if (isNowFavorited) {
      favoriteMediaIds.value.add(mediaId);
    } else {
      favoriteMediaIds.value.delete(mediaId);
    }
    // Also sync with TMDB if connected
    if (tmdbSessionId.value) {
      await toggleTmdbFavorite(mediaId, mediaType, isNowFavorited);
    }
    await loadFavorites();
  } catch (error) {
    console.error('Toggle favorite error:', error);
  }
};
const isFavorite = (mediaId: number) => {
  return favoriteMediaIds.value.has(mediaId);
};
const loadFavorites = async () => {
  try {
    const items: FavoriteItem[] = await invoke('get_favorites');
    favoriteItems.value = items;
    favoriteMediaIds.value = new Set(items.map(item => item.media_id));
  } catch (error) {
    console.error('Load favorites error:', error);
  }
};
// --- List Functions --- //
const loadListPreviews = async () => {
  const previews: Record<number, ListItem[]> = {};
  await Promise.all(userLists.value.map(async (list) => {
    try {
      const items: ListItem[] = await invoke('get_list_items', { listId: list.list_id });
      previews[list.list_id] = items.slice(0, 6);
    } catch (error) {
      console.error(`Load preview for list ${list.name} failed:`, error);
      previews[list.list_id] = [];
    }
  }));
  listPreviewItems.value = previews;
};
const loadLists = async () => {
  try {
    const lists: UserList[] = await invoke('get_lists');
    userLists.value = lists;
    await loadListPreviews();
  } catch (error) {
    console.error('Load lists error:', error);
  }
};
const createNewList = async () => {
  if (!newListName.value.trim()) return;
  try {
    await invoke('create_list', { name: newListName.value.trim() });
    newListName.value = '';
    showCreateListModal.value = false;
    await loadLists();
  } catch (error) {
    console.error('Create list error:', error);
  }
};
const openRenameList = (list: UserList) => {
  listToRename.value = list;
  renameListName.value = list.name;
  showRenameListModal.value = true;
};
const confirmRenameList = async () => {
  if (!listToRename.value || !renameListName.value.trim()) return;
  try {
    await invoke('rename_list', { listId: listToRename.value.list_id, name: renameListName.value.trim() });
    showRenameListModal.value = false;
    listToRename.value = null;
    await loadLists();
  } catch (error) {
    console.error('Rename list error:', error);
  }
};
const confirmDeleteList = async () => {
  if (!listToDelete.value) return;
  try {
    await invoke('delete_list', { listId: listToDelete.value.list_id });
    showDeleteListConfirm.value = false;
    listToDelete.value = null;
    if (selectedList.value?.list_id === listToDelete.value?.list_id) {
      selectedList.value = null;
      currentListPage.value = 'lists';
    }
    await loadLists();
  } catch (error) {
    console.error('Delete list error:', error);
  }
};
const viewList = async (list: UserList) => {
  selectedList.value = list;
  currentPage.value = 'list-view';
  try {
    const items: ListItem[] = await invoke('get_list_items', { listId: list.list_id });
    selectedListItems.value = items;
  } catch (error) {
    console.error('Load list items error:', error);
  }
};
const addMediaToList = async (listId: number) => {
  if (!selectedMedia.value) return;
  try {
    await invoke('add_to_list', { listId, mediaId: selectedMedia.value.id, mediaType: selectedMedia.value.media_type });
    showAddToListModal.value = false;
    await loadLists();
  } catch (error) {
    console.error('Add to list error:', error);
  }
};
const removeMediaFromList = async (mediaId: number) => {
  if (!selectedList.value) return;
  try {
    await invoke('remove_from_list', { listId: selectedList.value.list_id, mediaId });
    selectedListItems.value = selectedListItems.value.filter(item => item.media_id !== mediaId);
    // Update count
    if (selectedList.value) {
      selectedList.value.item_count = selectedListItems.value.length;
    }
    await loadLists();
  } catch (error) {
    console.error('Remove from list error:', error);
  }
};
const goBackToLists = () => {
  currentPage.value = 'lists';
  selectedList.value = null;
  selectedListItems.value = [];
};
const loadUserProfile = async () => {
  try {
    const profile = await invoke('get_user_profile') as UserProfileData;
    userProfile.value = {
      name: profile?.name || '',
      avatar_data_url: profile?.avatar_data_url || null,
      cover_data_url: profile?.cover_data_url || null,
    };
    profileNameDraft.value = userProfile.value.name;
  } catch (error) {
    console.error('Load user profile error:', error);
  }
};
const persistUserProfile = async (next: UserProfileData) => {
  try {
    const saved = await invoke('save_user_profile', { profile: next }) as UserProfileData;
    userProfile.value = {
      name: saved?.name || '',
      avatar_data_url: saved?.avatar_data_url || null,
      cover_data_url: saved?.cover_data_url || null,
    };
  } catch (error) {
    console.error('Save user profile error:', error);
  }
};
const startEditProfileName = () => {
  profileNameDraft.value = userProfile.value.name;
  isEditingProfileName.value = true;
};
const saveProfileName = async () => {
  const name = profileNameDraft.value.trim();
  isEditingProfileName.value = false;
  await persistUserProfile({ ...userProfile.value, name });
};
const cancelEditProfileName = () => {
  profileNameDraft.value = userProfile.value.name;
  isEditingProfileName.value = false;
};
const readImageAsDataUrl = (file: File): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
const onAvatarSelected = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    const dataUrl = await readImageAsDataUrl(file);
    await persistUserProfile({ ...userProfile.value, avatar_data_url: dataUrl });
  } catch (error) {
    console.error('Avatar upload error:', error);
  } finally {
    input.value = '';
  }
};
const onCoverSelected = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    const dataUrl = await readImageAsDataUrl(file);
    await persistUserProfile({ ...userProfile.value, cover_data_url: dataUrl });
  } catch (error) {
    console.error('Cover upload error:', error);
  } finally {
    input.value = '';
  }
};
onMounted(async () => {
  await checkTmdbSession();
  await loadDashboard();
  await loadUpcomingEpisodes();
  await loadNextEpisodesToWatch();
  await loadUserProfile();
  await loadFavorites();
  await loadLists();
  try {
    await invoke('get_upcoming_movies', { page: 1 });
  } catch (error) {
    console.error('Get upcoming movies error:', error);
  }
});
</script>
<template>
  <div class="min-h-screen bg-white text-black p-6 pb-24" style="padding-top: 0;">
    <router-view v-if="route.path === '/lists'" />
    <template v-else>
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
      <div class="flex border-b-2 border-gray-200 mb-6 pt-4">
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
        <div class="mb-8 text-center">
          <h2 class="text-sm font-bold bg-gray-400 text-white rounded-full px-4 py-1 inline-block mb-4 mx-auto block w-max">WATCH LIST</h2>
          <div v-if="nextEpisodesToWatch.filter(ep => !ep.is_havent_started).length > 0" class="grid grid-cols-6 gap-2">
            <div
              v-for="ep in nextEpisodesToWatch.filter(ep => !ep.is_havent_started)"
              :key="`${ep.media_id}-${ep.season_number}-${ep.episode_number}`"
              class="flex flex-col items-center cursor-pointer relative"
              @click="openMediaDetails({ id: ep.media_id, media_type: 'tv', title: ep.title, poster_path: ep.poster_path }, 'tv')"
            >
              <img
                :src="ep.poster_path ? `https://image.tmdb.org/t/p/w185${ep.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="ep.title"
                class="w-full aspect-[2/3] object-cover rounded-lg"
              />
            </div>
          </div>
          <p v-else class="text-gray-400 text-center mb-8">No shows in progress.</p>
        </div>
        <!-- Haven't Started section -->
        <div class="text-center">
          <h2 class="text-sm font-bold bg-gray-400 text-white rounded-full px-4 py-1 inline-block mb-4 mx-auto block w-max">HAVEN'T STARTED</h2>
          <div v-if="nextEpisodesToWatch.filter(ep => ep.is_havent_started).length > 0" class="grid grid-cols-6 gap-2">
            <div
              v-for="ep in nextEpisodesToWatch.filter(ep => ep.is_havent_started)"
              :key="ep.media_id"
              class="flex flex-col items-center cursor-pointer relative"
              @click="openMediaDetails({ id: ep.media_id, media_type: 'tv', title: ep.title, poster_path: ep.poster_path }, 'tv')"
            >
              <img
                :src="ep.poster_path ? `https://image.tmdb.org/t/p/w185${ep.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="ep.title"
                class="w-full aspect-[2/3] object-cover rounded-lg"
              />
            </div>
          </div>
          <p v-else class="text-gray-400 text-center mb-8">No shows to start.</p>
        </div>
      </div>
      <div v-else-if="activeShowsTab === 'upcoming'">
        <div v-if="upcomingEpisodes.length > 0" class="grid grid-cols-6 gap-2">
          <div
            v-for="ep in upcomingEpisodes"
            :key="`${ep.media_id}-${ep.season_number}-${ep.episode_number}`"
            class="flex flex-col items-center cursor-pointer relative"
            @click="openMediaDetails({ id: ep.media_id, media_type: 'tv', title: ep.title }, 'tv')"
          >
            <div class="relative w-full">
              <img
                :src="ep.still_path ? `https://image.tmdb.org/t/p/w185${ep.still_path}` : (ep.poster_path ? `https://image.tmdb.org/t/p/w185${ep.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image')"
                :alt="ep.episode_name"
                class="w-full aspect-[2/3] object-cover rounded-lg"
              />
              <div class="absolute bottom-2 left-2 bg-black/70 text-white px-2 py-1 rounded-md">
                <span class="text-lg font-bold">{{ ep.days_until }}</span>
              <br>
                <span class="text-lg ">days</span>
              </div>
            </div>
            <p class="text-xs text-gray-600 mt-1 text-center line-clamp-2">{{ ep.title }}</p>
            <p class="text-xs text-gray-500 text-center">S{{ ep.season_number }} E{{ ep.episode_number }}</p>
          </div>
        </div>
        <p v-else class="text-gray-400 text-center mb-8">No upcoming episodes.</p>
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
        <!-- Heart Favorite Button -->
        <button
          @click.stop="toggleFavorite(selectedMedia.id, selectedMedia.media_type)"
          class="absolute top-20 right-4 flex items-center justify-center w-10 h-10 rounded-full"
          :class="isFavorite(selectedMedia.id) ? 'bg-red-500 text-white' : 'bg-black/30 text-white'"
        >
          <svg class="w-6 h-6" :fill="isFavorite(selectedMedia.id) ? 'currentColor' : 'none'" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z"/>
          </svg>
        </button>
        <!-- Add to List Button -->
        <button
          @click.stop="showAddToListModal = true"
          class="absolute top-20 right-16 flex items-center justify-center w-10 h-10 rounded-full bg-black/30 text-white"
        >
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <rect x="3" y="4" width="18" height="18" rx="2" ry="2" stroke-width="2"/>
            <line x1="16" y1="2" x2="16" y2="6" stroke-width="2"/>
            <line x1="8" y1="2" x2="8" y2="6" stroke-width="2"/>
            <line x1="3" y1="10" x2="21" y2="10" stroke-width="2"/>
            <line x1="12" y1="14" x2="12" y2="18" stroke-width="2"/>
            <line x1="10" y1="16" x2="14" y2="16" stroke-width="2"/>
          </svg>
        </button>
        <!-- Lists Button -->
        <button
          @click.stop="currentPage = 'lists'"
          class="absolute top-20 right-28 flex items-center justify-center w-10 h-10 rounded-full bg-black/30 text-white"
        >
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <rect x="3" y="4" width="18" height="18" rx="2" ry="2" stroke-width="2"/>
            <line x1="16" y1="2" x2="16" y2="6" stroke-width="2"/>
            <line x1="8" y1="2" x2="8" y2="6" stroke-width="2"/>
            <line x1="3" y1="10" x2="21" y2="10" stroke-width="2"/>
          </svg>
        </button>
        <div class="absolute bottom-0 left-0 right-0 p-6 flex gap-4 items-end">
          <div class="flex-1">
            <h1 class="text-3xl font-bold text-white mb-2">
              {{ selectedMedia.title || selectedMedia.name }}
            </h1>
            <p class="text-white/90 text-lg mb-2">
              {{ selectedMedia.runtime ? `${Math.floor(selectedMedia.runtime / 60)}h ${selectedMedia.runtime % 60}m` : '2h 49m' }} • {{ selectedMedia.genres?.map(g => g.name).join(', ') || 'Action, Adventure, Thriller' }}
            </p>
          </div>
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
    <!-- Profile Page -->
    <div v-else-if="currentPage === 'profile'" class="-mx-6">
      <input
        ref="avatarFileInput"
        type="file"
        accept="image/*"
        class="hidden"
        @change="onAvatarSelected"
      />
      <input
        ref="coverFileInput"
        type="file"
        accept="image/*"
        class="hidden"
        @change="onCoverSelected"
      />
      <!-- Cover + identity -->
      <div class="relative mb-2">
        <div
          class="relative h-44 bg-[#2a2a2a] overflow-hidden"
          :style="userProfile.cover_data_url
            ? { backgroundImage: `url(${userProfile.cover_data_url})`, backgroundSize: 'cover', backgroundPosition: 'center' }
            : undefined"
        >
          <div class="absolute inset-0 bg-black/25 pointer-events-none"></div>
          <div class="relative z-10 flex items-center justify-between px-6 py-4">
            <button class="w-10 h-10 rounded-full bg-yellow-400 flex items-center justify-center" type="button" aria-label="Notifications">
              <svg class="w-5 h-5 text-black" fill="currentColor" viewBox="0 0 24 24">
                <path d="M12 22c1.1 0 2-.9 2-2h-4a2 2 0 0 0 2 2zm6-6v-5c0-3.07-1.64-5.64-4.5-6.32V4a1.5 1.5 0 0 0-3 0v.68C7.63 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2z"/>
              </svg>
            </button>
            <div class="flex items-center gap-2">
              <button
                type="button"
                class="w-9 h-9 rounded-full bg-black/50 text-white flex items-center justify-center hover:bg-black/70"
                aria-label="Change cover photo"
                @click="coverFileInput?.click()"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7h3l2-3h8l2 3h3v12H3V7z"/>
                  <circle cx="12" cy="13" r="3.5" stroke-width="2"/>
                </svg>
              </button>
              <button class="p-2 text-white" type="button" aria-label="Menu">
                <svg class="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
                  <circle cx="5" cy="12" r="2"/>
                  <circle cx="12" cy="12" r="2"/>
                  <circle cx="19" cy="12" r="2"/>
                </svg>
              </button>
            </div>
          </div>
        </div>
        <div class="px-6 -mt-10 relative z-10">
          <div class="flex items-end gap-3">
            <div class="relative">
              <div class="w-24 h-24 rounded-full bg-gray-300 border-4 border-white overflow-hidden flex items-center justify-center">
                <img
                  v-if="userProfile.avatar_data_url"
                  :src="userProfile.avatar_data_url"
                  alt="Profile"
                  class="w-full h-full object-cover"
                />
                <svg v-else class="w-14 h-14 text-gray-500" fill="currentColor" viewBox="0 0 24 24">
                  <path d="M12 12c2.7 0 4.8-2.1 4.8-4.8S14.7 2.4 12 2.4 7.2 4.5 7.2 7.2 9.3 12 12 12zm0 2.4c-3.2 0-9.6 1.6-9.6 4.8V22h19.2v-2.8c0-3.2-6.4-4.8-9.6-4.8z"/>
                </svg>
              </div>
              <button
                type="button"
                class="absolute bottom-0 right-0 w-8 h-8 rounded-full bg-yellow-400 text-black flex items-center justify-center border-2 border-white shadow"
                aria-label="Change profile photo"
                @click="avatarFileInput?.click()"
              >
                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7h3l2-3h8l2 3h3v12H3V7z"/>
                  <circle cx="12" cy="13" r="3.5" stroke-width="2"/>
                </svg>
              </button>
            </div>
            <button
              type="button"
              class="mb-2 px-4 py-1.5 rounded-full border border-gray-800 bg-[#2a2a2a] text-white text-xs font-bold tracking-wide"
              @click="startEditProfileName"
            >
              EDIT
            </button>
          </div>
          <div class="mt-3 mb-4">
            <div v-if="isEditingProfileName" class="flex items-center gap-2">
              <input
                v-model="profileNameDraft"
                type="text"
                maxlength="40"
                placeholder="Your name"
                class="flex-1 border border-gray-300 rounded-lg px-3 py-2 text-lg font-bold focus:outline-none focus:ring-2 focus:ring-yellow-400"
                @keyup.enter="saveProfileName"
                @keyup.escape="cancelEditProfileName"
              />
              <button type="button" class="px-3 py-2 bg-yellow-400 rounded-lg font-semibold text-sm" @click="saveProfileName">Save</button>
              <button type="button" class="px-3 py-2 text-gray-500 text-sm" @click="cancelEditProfileName">Cancel</button>
            </div>
            <h1 v-else class="text-2xl font-bold">{{ userProfile.name || 'Your name' }}</h1>
          </div>
        </div>
      </div>
      <!-- Social counts (static) -->
      <div class="grid grid-cols-3 border-y border-gray-200 bg-white mb-6">
        <div class="py-3 text-center border-r border-gray-200">
          <p class="font-bold text-lg leading-none">...</p>
          <p class="text-xs text-gray-500 mt-1">following</p>
        </div>
        <div class="py-3 text-center border-r border-gray-200">
          <p class="font-bold text-lg leading-none">...</p>
          <p class="text-xs text-gray-500 mt-1">followers</p>
        </div>
        <div class="py-3 text-center">
          <p class="font-bold text-lg leading-none">...</p>
          <p class="text-xs text-gray-500 mt-1">comments</p>
        </div>
      </div>
      <div class="px-6 pb-8 space-y-8">
        <!-- Stats -->
        <section>
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-xl font-bold">Stats</h2>
            <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
            </svg>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div class="border border-gray-200 rounded-lg p-3 bg-white">
              <div class="flex items-center gap-2 mb-3 text-sm text-gray-600">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <rect x="2" y="7" width="20" height="15" rx="2" stroke-width="2"/>
                  <path d="M17 7V5a2 2 0 0 0-2-2H9a2 2 0 0 0-2 2v2" stroke-width="2"/>
                </svg>
                <span>TV time</span>
              </div>
              <div class="flex justify-between text-center">
                <div>
                  <p class="text-lg font-bold leading-none">0</p>
                  <p class="text-[10px] text-gray-400 uppercase mt-1">Months</p>
                </div>
                <div>
                  <p class="text-lg font-bold leading-none">1</p>
                  <p class="text-[10px] text-gray-400 uppercase mt-1">Day</p>
                </div>
                <div>
                  <p class="text-lg font-bold leading-none">17</p>
                  <p class="text-[10px] text-gray-400 uppercase mt-1">Hours</p>
                </div>
              </div>
            </div>
            <div class="border border-gray-200 rounded-lg p-3 bg-white">
              <div class="flex items-center gap-2 mb-3 text-sm text-gray-600">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <rect x="3" y="4" width="18" height="18" rx="2" stroke-width="2"/>
                  <path d="M16 2v4M8 2v4M3 10h18" stroke-width="2"/>
                  <path d="M9 14l2 2 4-4" stroke-width="2"/>
                </svg>
                <span class="truncate">Episodes watched</span>
              </div>
              <p class="text-3xl font-bold">{{ profileEpisodesWatched }}</p>
            </div>
          </div>
        </section>
        <!-- Shows (completed) -->
<section class="w-full">
  <button
    @click="currentPage = 'lists'"
    class="w-full flex flex-col justify-between p-4 bg-white border border-gray-200 rounded-3xl hover:bg-gray-50 hover:border-blue-300 transition-all duration-200 cursor-pointer text-left h-64 shadow-sm group"
  >
    <!-- الجزء العلوي: العنوان والسهم -->
    <div class="w-full flex items-center justify-between">
      <h2 class="text-xl font-bold text-gray-800 group-hover:text-blue-600 transition-colors">
        Lists
      </h2>
      <svg class="w-5 h-5 text-gray-400 group-hover:text-blue-500 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
      </svg>
    </div>
    <!-- الجزء السفلي: مستطيل داخلي كـ Placeholder أو مساحة مخصصة للـ Preview -->
    <div class="w-full flex-1 mt-4 bg-gray-50 rounded-2xl border border-dashed border-gray-200 flex items-center justify-center text-gray-400 text-sm">
      <!-- لو حابب تحط داتا مستقبلاً هتكون هنا، حالياً مجرد مساحة للمظهر الرأسي -->
      <span>No items yet</span>
    </div>
  </button>
</section>
        <section>
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-xl font-bold">Shows</h2>
            <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
            </svg>
          </div>
          <div v-if="profileCompletedShows.length > 0" class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div
              v-for="[media] in profileCompletedShows"
              :key="media.media_id"
              class="flex-shrink-0 w-24 cursor-pointer"
              @click="openMediaDetails(media, 'tv')"
            >
              <img
                :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="media.title"
                class="w-full aspect-[2/3] object-cover rounded-md bg-gray-200"
              />
            </div>
          </div>
          <div v-else class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div v-for="n in 4" :key="n" class="flex-shrink-0 w-24 aspect-[2/3] rounded-md bg-gray-200"></div>
          </div>
        </section>
        <!-- Favorite shows -->
        <section>
          <div class="flex items-center gap-2 mb-3">
            <svg class="w-5 h-5 text-red-500" fill="currentColor" viewBox="0 0 24 24">
              <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z"/>
            </svg>
            <h2 class="text-xl font-bold">Favorite shows</h2>
          </div>
          <div v-if="favoriteShows.length > 0" class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div
              v-for="item in favoriteShows"
              :key="item.media_id"
              class="flex-shrink-0 w-24 cursor-pointer"
              @click="openMediaDetails({ id: item.media_id, media_type: 'tv', title: item.title, poster_path: item.poster_path }, 'tv')"
            >
              <img
                :src="item.poster_path ? `https://image.tmdb.org/t/p/w185${item.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="item.title"
                class="w-full aspect-[2/3] object-cover rounded-md bg-gray-200"
              />
            </div>
          </div>
          <div v-else class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div v-for="n in 4" :key="n" class="flex-shrink-0 w-24 aspect-[2/3] rounded-md bg-gray-200"></div>
          </div>
        </section>
        <!-- Movies (watched) -->
        <section>
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-xl font-bold">Movies</h2>
            <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
            </svg>
          </div>
          <div v-if="profileWatchedMovies.length > 0" class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div
              v-for="[media] in profileWatchedMovies"
              :key="media.media_id"
              class="flex-shrink-0 w-24 cursor-pointer"
              @click="openMediaDetails(media)"
            >
              <img
                :src="media.poster_path ? `https://image.tmdb.org/t/p/w185${media.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="media.title"
                class="w-full aspect-[2/3] object-cover rounded-md bg-gray-200"
              />
            </div>
          </div>
          <div v-else class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div v-for="n in 4" :key="n" class="flex-shrink-0 w-24 aspect-[2/3] rounded-md bg-gray-200"></div>
          </div>
        </section>
        <!-- Favorite movies -->
        <section>
          <div class="flex items-center gap-2 mb-3">
            <svg class="w-5 h-5 text-red-500" fill="currentColor" viewBox="0 0 24 24">
              <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z"/>
            </svg>
            <h2 class="text-xl font-bold">Favorite movies</h2>
          </div>
          <div v-if="favoriteMovies.length > 0" class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div
              v-for="item in favoriteMovies"
              :key="item.media_id"
              class="flex-shrink-0 w-24 cursor-pointer"
              @click="openMediaDetails({ id: item.media_id, media_type: 'movie', title: item.title, poster_path: item.poster_path }, 'movie')"
            >
              <img
                :src="item.poster_path ? `https://image.tmdb.org/t/p/w185${item.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
                :alt="item.title"
                class="w-full aspect-[2/3] object-cover rounded-md bg-gray-200"
              />
            </div>
          </div>
          <div v-else class="flex gap-2 overflow-x-auto scrollbar-hide pb-1">
            <div v-for="n in 4" :key="n" class="flex-shrink-0 w-24 aspect-[2/3] rounded-md bg-gray-200"></div>
          </div>
        </section>
      </div>
    </div>
    <!-- Lists Page -->
    <div v-else-if="currentPage === 'lists'" class="-mx-6">
      <div class="px-6 pt-4 pb-4 border-b border-gray-200 flex items-center gap-4">
        <button @click="currentPage = 'home'" class="p-2 hover:bg-gray-100 rounded-full">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
          </svg>
        </button>
        <h1 class="text-xl font-bold flex-1">Lists</h1>
        <button @click="showCreateListModal = true" class="text-gray-400 hover:text-gray-600">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <line x1="12" y1="5" x2="12" y2="19" stroke-width="2"/>
            <line x1="5" y1="12" x2="19" y2="12" stroke-width="2"/>
          </svg>
        </button>
      </div>
      <div class="px-6 py-4 space-y-4">
        <div v-if="userLists.length > 0" class="space-y-4">
          <div
            v-for="list in userLists"
            :key="list.list_id"
            class="bg-white border border-gray-200 rounded-2xl p-6 hover:border-gray-400 hover:shadow-md transition-all cursor-pointer"
            @click="viewList(list)"
          >
            <div class="flex items-center justify-between gap-4 mb-4">
              <div class="flex-1">
                <p class="text-xl font-bold text-gray-900">{{ list.name }}</p>
                <p class="text-sm text-gray-500 mt-1">{{ list.item_count }} items</p>
              </div>
              <div class="flex items-center gap-2 text-gray-400" @click.stop>
                <button @click="openRenameList(list)" class="p-2 rounded-full hover:bg-gray-100" title="Rename">
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
                  </svg>
                </button>
                <button @click="listToDelete = list; showDeleteListConfirm = true" class="p-2 rounded-full hover:bg-gray-100" title="Delete">
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                  </svg>
                </button>
              </div>
            </div>
            <div v-if="listPreviewItems[list.list_id] && listPreviewItems[list.list_id].length > 0" class="flex gap-2 overflow-x-auto scrollbar-hide">
              <template v-for="item in listPreviewItems[list.list_id]" :key="item.media_id">
                <img
                  :src="item.poster_path ? `https://image.tmdb.org/t/p/w185${item.poster_path}` : 'https://via.placeholder.com/100x150?text=No+Image'"
                  :alt="item.title"
                  class="w-[200px] h-[300px] object-cover rounded-lg flex-shrink-0"
                />
              </template>
            </div>
          </div>
        </div>
        <div v-else class="text-center text-gray-400 py-10">
          <p>No lists yet. Tap + to create one.</p>
        </div>
      </div>
    </div>
    <!-- List View Page (when clicking a list from profile) -->
    <div v-else-if="currentPage === 'list-view' && selectedList" class="-mx-6">
      <div class="px-6 pt-4 pb-4 border-b border-gray-200 flex items-center gap-4">
        <button @click="goBackToLists" class="p-2 hover:bg-gray-100 rounded-full">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
          </svg>
        </button>
        <h1 class="text-xl font-bold flex-1">{{ selectedList.name }}</h1>
        <span class="text-sm text-gray-500">{{ selectedList.item_count }} items</span>
      </div>
      <div class="px-6 py-4">
        <div v-if="selectedListItems.length > 0" class="grid grid-cols-6 gap-3">
          <div
            v-for="item in selectedListItems"
            :key="item.media_id"
            class="cursor-pointer relative"
            @click="openMediaDetails({ id: item.media_id, media_type: item.media_type, title: item.title, poster_path: item.poster_path }, item.media_type)"
          >
            <img
              :src="item.poster_path ? `https://image.tmdb.org/t/p/w185${item.poster_path}` : 'https://via.placeholder.com/200x300?text=No+Image'"
              :alt="item.title"
              class="w-full aspect-[2/3] object-cover rounded-xl"
            />
            <button
              @click.stop="removeMediaFromList(item.media_id)"
              class="absolute -top-2 -right-2 w-6 h-6 bg-red-500 text-white rounded-full flex items-center justify-center z-10"
            >
              <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <line x1="18" y1="6" x2="6" y2="18" stroke-width="2"/>
                <line x1="6" y1="6" x2="18" y2="18" stroke-width="2"/>
              </svg>
            </button>
          </div>
        </div>
        <div v-else class="text-center text-gray-400 py-10">
          <!-- <p>This list is empty.</p> -->
        </div>
      </div>
    </div>
    <!-- Create List Modal -->
    <div v-if="showCreateListModal" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
      <div class="bg-white rounded-lg p-6 max-w-sm w-full">
        <h3 class="text-lg font-bold mb-4">Create New List</h3>
        <input
          v-model="newListName"
          type="text"
          placeholder="List name..."
          maxlength="50"
          class="w-full border border-gray-300 rounded-lg px-3 py-2 mb-4 focus:outline-none focus:ring-2 focus:ring-yellow-400"
          @keyup.enter="createNewList"
        />
        <div class="flex gap-3">
          <button @click="showCreateListModal = false; newListName = ''" class="flex-1 bg-gray-200 hover:bg-gray-300 text-black font-semibold py-3 rounded-lg">Cancel</button>
          <button @click="createNewList" class="flex-1 bg-yellow-400 hover:bg-yellow-500 text-black font-semibold py-3 rounded-lg">Create</button>
        </div>
      </div>
    </div>
    <!-- Rename List Modal -->
    <div v-if="showRenameListModal" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
      <div class="bg-white rounded-lg p-6 max-w-sm w-full">
        <h3 class="text-lg font-bold mb-4">Rename List</h3>
        <input
          v-model="renameListName"
          type="text"
          placeholder="List name..."
          maxlength="50"
          class="w-full border border-gray-300 rounded-lg px-3 py-2 mb-4 focus:outline-none focus:ring-2 focus:ring-yellow-400"
          @keyup.enter="confirmRenameList"
        />
        <div class="flex gap-3">
          <button @click="showRenameListModal = false; listToRename = null" class="flex-1 bg-gray-200 hover:bg-gray-300 text-black font-semibold py-3 rounded-lg">Cancel</button>
          <button @click="confirmRenameList" class="flex-1 bg-yellow-400 hover:bg-yellow-500 text-black font-semibold py-3 rounded-lg">Save</button>
        </div>
      </div>
    </div>
    <!-- Delete List Confirmation -->
    <div v-if="showDeleteListConfirm" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
      <div class="bg-white rounded-lg p-6 max-w-sm w-full">
        <h3 class="text-lg font-bold mb-4">Delete List?</h3>
        <p class="text-gray-600 mb-6">Are you sure you want to delete "{{ listToDelete?.name }}"? This cannot be undone.</p>
        <div class="flex gap-3">
          <button @click="showDeleteListConfirm = false; listToDelete = null" class="flex-1 bg-gray-200 hover:bg-gray-300 text-black font-semibold py-3 rounded-lg">Cancel</button>
          <button @click="confirmDeleteList" class="flex-1 bg-red-500 hover:bg-red-600 text-white font-semibold py-3 rounded-lg">Delete</button>
        </div>
      </div>
    </div>
    <!-- Add to List Modal (from details page) -->
    <div v-if="showAddToListModal" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
      <div class="bg-white rounded-lg p-6 max-w-sm w-full max-h-[80vh] overflow-y-auto">
        <h3 class="text-lg font-bold mb-4">Add to List</h3>
        <div v-if="userLists.length > 0" class="space-y-2 mb-4">
          <button
            v-for="list in userLists"
            :key="list.list_id"
            @click="addMediaToList(list.list_id)"
            class="w-full text-left p-3 rounded-lg border border-gray-200 hover:bg-gray-50 transition-colors"
          >
            <p class="font-semibold">{{ list.name }}</p>
            <p class="text-xs text-gray-500">{{ list.item_count }} items</p>
          </button>
        </div>
        <div v-else class="text-center text-gray-400 py-4 mb-4">
          <p>No lists yet. Create one from your profile.</p>
        </div>
        <button @click="showAddToListModal = false" class="w-full bg-gray-200 hover:bg-gray-300 text-black font-semibold py-3 rounded-lg">Cancel</button>
      </div>
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
        @click="currentPage = 'lists'"
        :class="currentPage === 'lists' ? 'text-blue-600' : 'text-gray-400'"
        class="flex flex-col items-center gap-1 hover:text-gray-600 transition-colors"
      >
        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <rect x="3" y="4" width="18" height="18" rx="2" ry="2" stroke-width="2"/>
          <line x1="16" y1="2" x2="16" y2="6" stroke-width="2"/>
          <line x1="8" y1="2" x2="8" y2="6" stroke-width="2"/>
          <line x1="3" y1="10" x2="21" y2="10" stroke-width="2"/>
        </svg>
        <span class="text-xs font-medium">Lists</span>
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
    </template>
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
