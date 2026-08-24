<script setup lang="ts">
import { computed } from "vue";
import { Trash2 } from "lucide-vue-next";
import type { MusicFile, TrackMetadata } from "../types";

const props = defineProps<{
  files: MusicFile[];
  tracks: TrackMetadata[];
  text: Record<string, string>;
  pendingDeleteIndex: number | null;
  displayTargetFileName: (index: number) => string;
}>();

const emit = defineEmits<{
  (event: "target-file-name-input", index: number, value: string): void;
  (event: "track-input", index: number, field: "title" | "artist", value: string): void;
  (event: "request-remove", index: number): void;
  (event: "remove", index: number): void;
  (event: "cancel-remove"): void;
}>();

const rows = computed(() => {
  const max = Math.max(props.files.length, props.tracks.length);
  return Array.from({ length: max }, (_, index) => ({
    file: props.files[index],
    track: props.tracks[index],
  }));
});
</script>

<template>
  <div v-if="rows.length" class="table-wrap">
    <table>
      <thead>
        <tr>
          <th>{{ text.targetFile }}</th>
          <th>{{ text.number }}</th>
          <th>{{ text.title }}</th>
          <th>{{ text.artist }}</th>
          <th>{{ text.actions }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(row, index) in rows" :key="index">
          <td class="file-name-cell">
            <input
              v-if="row.file"
              class="table-input file-name-input"
              :value="displayTargetFileName(index)"
              :aria-label="text.targetFile"
              @input="emit('target-file-name-input', index, ($event.target as HTMLInputElement).value)"
            />
            <span v-else>-</span>
          </td>
          <td>{{ row.track?.number || "-" }}</td>
          <td>
            <input
              class="table-input"
              :value="tracks[index]?.title ?? ''"
              :aria-label="text.title"
              @input="emit('track-input', index, 'title', ($event.target as HTMLInputElement).value)"
            />
          </td>
          <td>
            <input
              class="table-input"
              :value="tracks[index]?.artist ?? ''"
              :aria-label="text.artist"
              @input="emit('track-input', index, 'artist', ($event.target as HTMLInputElement).value)"
            />
          </td>
          <td class="actions-cell">
            <div
              v-if="row.track"
              class="delete-popover-wrap"
              :class="{
                'delete-popover-wrap-active': pendingDeleteIndex === index,
                'delete-popover-wrap-below': index < 2,
              }"
            >
              <button
                class="row-action-button"
                type="button"
                :title="text.deleteTrack"
                :aria-expanded="pendingDeleteIndex === index"
                @click="emit('request-remove', index)"
              >
                <Trash2 :size="16" aria-hidden="true" />
              </button>
              <div
                v-if="pendingDeleteIndex === index"
                class="delete-popover"
                :class="{ 'delete-popover-below': index < 2 }"
              >
                <span>{{ text.confirmDeleteTrack }}</span>
                <div class="delete-popover-actions">
                  <button class="confirm-delete-button" type="button" @click="emit('remove', index)">
                    {{ text.deleteTrack }}
                  </button>
                  <button class="cancel-delete-button" type="button" @click="emit('cancel-remove')">
                    {{ text.cancel }}
                  </button>
                </div>
              </div>
            </div>
            <span v-else>-</span>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
  <p v-else class="empty-preview">{{ text.emptyPreview }}</p>
</template>

<style scoped>
.table-wrap {
  overflow: auto;
}

table {
  border-collapse: collapse;
  table-layout: fixed;
  width: 100%;
}

th:nth-child(1), td:nth-child(1) { width: 31%; }
th:nth-child(2), td:nth-child(2) { width: 56px; }
th:nth-child(3), td:nth-child(3) { width: 29%; }
th:nth-child(4), td:nth-child(4) { width: calc(40% - 112px); }
th:nth-child(5), td:nth-child(5) { width: 56px; }

th,
td {
  border-bottom: 1px solid #e1e8ec;
  font-size: 13px;
  line-height: 1.4;
  padding: 10px 12px;
  text-align: left;
  vertical-align: middle;
}

th {
  background: #f7fafb;
  color: #4b535e;
  font-weight: 700;
  position: sticky;
  top: 0;
  white-space: nowrap;
  z-index: 8;
}

td { color: #20252b; overflow-wrap: anywhere; }

.table-input {
  background: #ffffff;
  border: 1px solid transparent;
  border-radius: 6px;
  color: #20252b;
  font: inherit;
  min-width: 0;
  outline: none;
  padding: 6px 7px;
  width: 100%;
}

.table-input:focus {
  border-color: #267a86;
  box-shadow: 0 0 0 3px rgba(38, 122, 134, 0.12);
}

.actions-cell { text-align: center; }

.row-action-button {
  align-items: center;
  background: #f6f8f9;
  border: 1px solid #d9e1e5;
  border-radius: 7px;
  color: #65717d;
  cursor: pointer;
  display: inline-flex;
  height: 32px;
  justify-content: center;
  width: 32px;
}

.row-action-button:hover { background: #eef2f4; color: #a13b2a; }

.delete-popover-wrap { display: inline-flex; justify-content: center; position: relative; }
.delete-popover-wrap-active { z-index: 12; }

.delete-popover {
  background: #ffffff;
  border: 1px solid #d9e1e5;
  border-radius: 8px;
  box-shadow: 0 12px 34px rgba(29, 36, 43, 0.16);
  color: #252a31;
  display: grid;
  gap: 10px;
  min-width: 176px;
  padding: 10px;
  position: absolute;
  bottom: calc(100% + 8px);
  right: 0;
  z-index: 6;
}

.delete-popover-below { bottom: auto; top: calc(100% + 8px); }
.delete-popover::before {
  background: #ffffff;
  border-left: 1px solid #d9e1e5;
  border-top: 1px solid #d9e1e5;
  bottom: -5px;
  content: "";
  height: 9px;
  position: absolute;
  right: 12px;
  transform: rotate(225deg);
  width: 9px;
}
.delete-popover-below::before { bottom: auto; top: -5px; transform: rotate(45deg); }
.delete-popover span { font-size: 13px; line-height: 1.4; text-align: left; }
.delete-popover-actions { display: flex; gap: 8px; justify-content: flex-end; }

.confirm-delete-button,
.cancel-delete-button {
  border: 0;
  border-radius: 6px;
  cursor: pointer;
  font: inherit;
  font-size: 13px;
  min-height: 30px;
  padding: 6px 10px;
}

.confirm-delete-button { background: #a13b2a; color: #ffffff; }
.confirm-delete-button:hover { background: #8c3123; }
.cancel-delete-button { background: #eef2f4; color: #48515b; }
.cancel-delete-button:hover { background: #e3e9ec; }

.empty-preview { color: #68717d; margin: 0; padding: 22px 20px; }
</style>
