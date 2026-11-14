import { beforeEach, describe, expect, it } from 'vitest';
import { counts, libraryItems, useApp, visibleTasks } from './store';
import type { MediaView, TaskView } from './types';

const task = (id: string, over: Partial<TaskView> = {}): TaskView => ({
  id, kind: 'http', fileName: `${id}.zip`, directory: 'C:\\D', host: 'example.com', displayUrl: 'https://example.com/x',
  status: 'downloading', totalBytes: 100, completedBytes: 10, speed: 5, connections: 2, etaSeconds: 18,
  resumable: true, contentType: null, error: null, retryCount: 0, nextRetryAt: null,
  createdAt: Number(id.replace(/\D/g, '')) || 1, startedAt: null, finishedAt: null,
  category: 'archive', tags: [], sha256: null, expectedSha256: null, speedLimit: 0, priority: 0, pausedReason: null,
  uploadSpeed: 0, uploadedBytes: 0, seeders: 0, duplicate: false, ...over
});

const media = (id: string, over: Partial<MediaView> = {}): MediaView => ({
  id, kind: 'convert', title: 'aula.mkv', source: 'C:\\D\\aula.mkv', status: 'completed', progress: 1, phase: '', speed: null, eta: null,
  totalBytes: 50, outputPath: 'C:\\D\\aula (convertido).mp3', outputDir: 'C:\\D', thumbnail: null, duration: 60, label: 'MP3',
  item: null, items: null, error: null, createdAt: 5, finishedAt: 9, ...over
});

describe('store', () => {
  beforeEach(() => useApp.setState({ tasks: {}, media: {}, selectedId: null }));

  it('eventos atualizam e removem tarefas; seleção some com a tarefa removida', () => {
    const s = useApp.getState();
    s.upsertTasks([task('t1'), task('t2')]);
    s.upsertTasks([task('t1', { completedBytes: 90 })]);
    expect(useApp.getState().tasks.t1.completedBytes).toBe(90);
    s.select('t2');
    s.removeTasks(['t2']);
    expect(useApp.getState().tasks.t2).toBeUndefined();
    expect(useApp.getState().selectedId).toBeNull();
    s.upsertMedia([media('m1')]);
    s.removeMedia(['m1']);
    expect(useApp.getState().media.m1).toBeUndefined();
  });

  it('filtros, busca (inclui etiquetas) e ordem estável', () => {
    const tasks = Object.fromEntries(
      [
        task('t1', { status: 'completed', tags: ['trabalho'] }),
        task('t2', { status: 'failed', host: 'cdn.org' }),
        task('t3', { status: 'paused', kind: 'magnet' }),
        task('t4')
      ].map((t) => [t.id, t])
    );
    expect(visibleTasks(tasks, 'all', '').map((t) => t.id)).toEqual(['t4', 't3', 't2', 't1']);
    expect(visibleTasks(tasks, 'active', '').map((t) => t.id)).toEqual(['t4', 't3']);
    expect(visibleTasks(tasks, 'torrents', '').map((t) => t.id)).toEqual(['t3']);
    expect(visibleTasks(tasks, 'all', 'CDN').map((t) => t.id)).toEqual(['t2']);
    expect(visibleTasks(tasks, 'all', 'trab').map((t) => t.id)).toEqual(['t1']);
    expect(counts(tasks)).toEqual({ all: 4, active: 2, completed: 1, failed: 1, torrents: 1 });
  });

  it('biblioteca junta downloads concluídos e mídia convertida', () => {
    const tasks = { a: task('a1', { id: 'a', status: 'completed', finishedAt: 10, category: 'video', fileName: 'f.mp4' }), b: task('b2', { id: 'b' }) };
    const items = libraryItems(tasks, { m: media('m') });
    expect(items.map((i) => i.key)).toEqual(['d:a', 'm:m']);
    expect(items[1]).toMatchObject({ name: 'aula (convertido).mp3', category: 'audio', origin: 'MP3' });
  });
});
