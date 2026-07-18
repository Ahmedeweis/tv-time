<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke as tauriInvoke } from '@tauri-apps/api/core';

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
const listPreviewItems = ref<Record<number, ListItem[]>>({});
const selectedList = ref<UserList | null>(null);
const selectedListItems = ref<ListItem[]>([]);
const showCreateListModal = ref(false);
const showRenameListModal = ref(false);
const showDeleteListConfirm = ref(false);
const newListName = ref('');
const renameListName = ref('');
const listToRename = ref<UserList | null>(null);
const listToDelete = ref<UserList | null>(null);

const invoke = async <T = unknown>(cmd: string, args?: any): Promise<T> => {
  try {
    const result = await tauriInvoke(cmd, args);
    return result as T;
  } catch (error) {
    console.error(`[ListsPage] ${cmd} failed:`, error);
    throw error;
  }
};

const loadListPreviews = async () => {
  const previews: Record<number, ListItem[]> = {};
  await Promise.all(
    userLists.value.map(async (list) => {
      try {
        const items = await invoke<ListItem[]>('get_list_items', { listId: list.list_id });
        previews[list.list_id] = items.slice(0, 6);
      } catch (error) {
        console.error(`Load preview for list ${list.name} failed:`, error);
        previews[list.list_id] = [];
      }
    }),
  );
  listPreviewItems.value = previews;
};

const loadLists = async () => {
  try {
    const lists = await invoke<UserList[]>('get_lists');
    userLists.value = lists;
    await loadListPreviews();
  } catch (error) {
    console.error('Load lists error:', error);
  }
};

const viewList = async (list: UserList) => {
  selectedList.value = list;
  selectedListItems.value = [];
  try {
    selectedListItems.value = await invoke<ListItem[]>('get_list_items', { listId: list.list_id });
  } catch (error) {
    console.error('Load list items error:', error);
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
  const deletingId = listToDelete.value.list_id;
  try {
    await invoke('delete_list', { listId: deletingId });
    showDeleteListConfirm.value = false;
    listToDelete.value = null;
    if (selectedList.value?.list_id === deletingId) {
      selectedList.value = null;
      selectedListItems.value = [];
    }
    await loadLists();
  } catch (error) {
    console.error('Delete list error:', error);
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

onMounted(loadLists);
</script>

<template>
  <div class="min-h-screen bg-white text-black p-6 pb-24" style="padding-top: 0;">
    <div class="max-w-5xl mx-auto">
      <div class="flex items-center justify-between gap-4 mb-6">
        <div>
          <h1 class="text-3xl font-bold">Lists</h1>
          <p class="text-sm text-gray-500 mt-1">Browse your collections and preview up to six items per list.</p>
        </div>
        <button
          @click="showCreateListModal = true"
          class="rounded-full bg-yellow-400 px-4 py-2 text-sm font-semibold text-black hover:bg-yellow-500 transition-colors"
        >
          New list
        </button>
      </div>

      <div v-if="userLists.length > 0" class="space-y-4">
        <div
          v-for="list in userLists"
          :key="list.list_id"
          class="bg-gray-50 border border-gray-200 rounded-3xl p-4 hover:bg-gray-100 transition-colors cursor-pointer"
          @click="viewList(list)"
        >
          <div class="flex items-start justify-between gap-4">
            <div>
              <p class="text-lg font-semibold">{{ list.name }}</p>
              <p class="text-xs text-gray-500 mt-1">{{ list.item_count }} items</p>
            </div>
            <div class="flex items-center gap-2" @click.stop>
              <button
                @click="openRenameList(list)"
                class="rounded-full p-2 text-gray-500 hover:bg-white hover:text-blue-600 transition-colors"
                title="Rename"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
                </svg>
              </button>
              <button
                @click="listToDelete = list; showDeleteListConfirm = true"
                class="rounded-full p-2 text-gray-500 hover:bg-white hover:text-red-600 transition-colors"
                title="Delete"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                </svg>
              </button>
            </div>
          </div>

          <div class="grid grid-cols-3 gap-2 mt-4">
            <template v-for="item in listPreviewItems[list.list_id] || []" :key="item.media_id">
              <img
                :src="item.poster_path ? `https://image.tmdb.org/t/p/w185${item.poster_path}` : 'https://via.placeholder.com/100x150?text=No+Image'"
                :alt="item.title"
                class="w-full aspect-[2/3] object-cover rounded-xl"
              />
            </template>
            <template v-if="(listPreviewItems[list.list_id] || []).length === 0">
              <div class="col-span-3 rounded-3xl border border-dashed border-gray-300 bg-white p-6 text-center text-xs text-gray-500">
                No preview available
              </div>
            </template>
          </div>
        </div>
      </div>

      <div v-if="selectedList" class="bg-white border border-gray-200 rounded-3xl p-4 mt-6">
        <div class="flex items-center justify-between gap-4 mb-4">
          <div>
            <h2 class="text-xl font-semibold">{{ selectedList.name }}</h2>
            <p class="text-sm text-gray-500">Showing {{ selectedListItems.length }} items</p>
          </div>
          <button
            @click="selectedList = null; selectedListItems = []"
            class="rounded-full bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200"
          >
            Close
          </button>
        </div>
        <div class="space-y-3">
          <div
            v-for="item in selectedListItems"
            :key="item.media_id"
            class="flex items-center gap-4 bg-gray-50 border border-gray-200 rounded-3xl p-4"
          >
            <img
              :src="item.poster_path ? `https://image.tmdb.org/t/p/w185${item.poster_path}` : 'https://via.placeholder.com/100x150?text=No+Image'"
              :alt="item.title"
              class="w-20 aspect-[2/3] object-cover rounded-xl"
            />
            <div class="flex-1">
              <p class="font-semibold">{{ item.title }}</p>
              <p class="text-xs text-gray-500">{{ item.media_type === 'movie' ? 'Movie' : 'TV Show' }} • {{ item.release_date || 'Unknown date' }}</p>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="text-center text-gray-400 py-16">
        <p class="text-lg">No lists yet.</p>
        <p class="mt-2">Create a new list to begin organizing your movies and shows.</p>
      </div>
    </div>

    <div v-if="showCreateListModal" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
      <div class="bg-white rounded-3xl p-6 w-full max-w-md">
        <h2 class="text-xl font-semibold mb-4">Create New List</h2>
        <input
          v-model="newListName"
          type="text"
          placeholder="List name"
          class="w-full rounded-2xl border border-gray-200 px-4 py-3 focus:outline-none focus:ring-2 focus:ring-yellow-400"
        />
        <div class="mt-4 flex gap-3">
          <button @click="showCreateListModal = false; newListName = ''" class="flex-1 rounded-2xl bg-gray-100 px-4 py-3 text-sm font-semibold">Cancel</button>
          <button @click="createNewList" class="flex-1 rounded-2xl bg-yellow-400 px-4 py-3 text-sm font-semibold text-black">Create</button>
        </div>
      </div>
    </div>

    <div v-if="showRenameListModal" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
      <div class="bg-white rounded-3xl p-6 w-full max-w-md">
        <h2 class="text-xl font-semibold mb-4">Rename List</h2>
        <input
          v-model="renameListName"
          type="text"
          placeholder="List name"
          class="w-full rounded-2xl border border-gray-200 px-4 py-3 focus:outline-none focus:ring-2 focus:ring-yellow-400"
        />
        <div class="mt-4 flex gap-3">
          <button @click="showRenameListModal = false; listToRename = null" class="flex-1 rounded-2xl bg-gray-100 px-4 py-3 text-sm font-semibold">Cancel</button>
          <button @click="confirmRenameList" class="flex-1 rounded-2xl bg-yellow-400 px-4 py-3 text-sm font-semibold text-black">Save</button>
        </div>
      </div>
    </div>

    <div v-if="showDeleteListConfirm" class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
      <div class="bg-white rounded-3xl p-6 w-full max-w-md">
        <h2 class="text-xl font-semibold mb-4">Delete List</h2>
        <p class="text-gray-600">Are you sure you want to delete "{{ listToDelete?.name }}"? This action cannot be undone.</p>
        <div class="mt-6 flex gap-3">
          <button @click="showDeleteListConfirm = false; listToDelete = null" class="flex-1 rounded-2xl bg-gray-100 px-4 py-3 text-sm font-semibold">Cancel</button>
          <button @click="confirmDeleteList" class="flex-1 rounded-2xl bg-red-500 px-4 py-3 text-sm font-semibold text-white">Delete</button>
        </div>
      </div>
    </div>
  </div>
</template>
