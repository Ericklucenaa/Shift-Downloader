import { open as openDialog } from '@tauri-apps/plugin-dialog';
import clsx from 'clsx';
import { ArrowRight, Film, FolderOpen, FolderSearch, Music, Pause, Play, RotateCcw, Search, SquareArrowOutUpRight, Trash2, Wand2, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { mediaActions, reportError } from '../lib/actions';
import { bytes, duration, percentLabel, speed } from '../lib/format';
import { PageTitle } from '../components/ui/page';
import { diagnosticText, useT } from '../lib/i18n';
import { api, asDiagnostic } from '../lib/ipc';
import { useApp } from '../lib/store';
import type { Diagnostic, FileProbe, MediaInfo, MediaView } from '../lib/types';
import { Button, IconButton, Spinner } from '../components/ui/primitives';
import { ProgressBar } from '../components/ui/progress';
import { TextInput } from '../components/ui/form';

function ErrorNote({ error }: { error: Diagnostic }) {
  const { lang } = useT();
  const d = diagnosticText(error, lang);
  return (
    <div role="alert" className="mt-3 rounded-md border border-bad/30 bg-bad-soft px-3 py-2 text-[12.5px]">
      <div className="font-medium text-ink">{d.title}</div>
      <div className="text-ink-2">{d.action}</div>
      {error.detail && <div className="num mt-0.5 text-[11.5px] break-all text-muted selectable">{error.detail}</div>}
    </div>
  );
}

function Analyzer() {
  const { t, lang } = useT();
  const settings = useApp((s) => s.settings);
  const pending = useApp((s) => s.pendingExternal);
  const setPending = useApp((s) => s.setPendingExternal);
  const [url, setUrl] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Diagnostic | null>(null);
  const [info, setInfo] = useState<{ url: string; data: MediaInfo } | null>(null);
  const [choice, setChoice] = useState('best');
  const [subs, setSubs] = useState(false);
  const [playlist, setPlaylist] = useState(true);
  const [dir, setDir] = useState<string | null>(null);

  const analyze = async (raw: string) => {
    const u = raw.trim();
    if (!u || busy) return;
    setUrl(u);
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      const data = await api.mediaAnalyze(u);
      setInfo({ url: u, data });
      setChoice(data.choices[0]?.id ?? 'best');
    } catch (e) {
      setError(asDiagnostic(e));
    } finally {
      setBusy(false);
    }
  };

  // Link enviado pela extensão do navegador ("baixar vídeo desta página").
  useEffect(() => {
    if (pending && pending.kind === 'media') {
      setPending(null);
      void analyze(pending.url);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pending]);

  const download = async () => {
    if (!info) return;
    const c = info.data.choices.find((x) => x.id === choice);
    try {
      await api.mediaAddDownload({
        url: info.url,
        choice,
        choiceLabel: c?.label,
        title: info.data.title,
        videoId: info.data.id,
        thumbnail: info.data.thumbnail,
        duration: info.data.duration,
        playlist: info.data.isPlaylist && playlist,
        subtitles: subs,
        directory: dir ?? undefined
      });
      setInfo(null);
      setUrl('');
    } catch (e) {
      reportError(e);
    }
  };

  return (
    <section className="rounded-lg border border-line bg-surface">
      <form
        className="flex items-center gap-2 p-3"
        onSubmit={(e) => {
          e.preventDefault();
          void analyze(url);
        }}
      >
        <div className="flex h-10 min-w-0 flex-1 items-center gap-2 rounded-md border border-line-strong bg-surface-2 px-3 focus-within:border-ink-2">
          {busy ? <Spinner /> : <Search size={15} className="text-muted" />}
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder={busy ? t('media.analyzing') : t('media.placeholder')}
            aria-label={t('media.placeholder')}
            spellCheck={false}
            disabled={busy}
            className="num h-full min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:font-sans placeholder:text-faint focus-visible:outline-none"
          />
        </div>
        <Button type="submit" variant="primary" className="h-10" disabled={!url.trim() || busy} icon={<ArrowRight size={14} />}>
          {t('media.analyze')}
        </Button>
      </form>
      {error && <div className="px-3 pb-3">{<ErrorNote error={error} />}</div>}
      {info && (
        <div className="border-t border-line p-4">
          <div className="flex gap-4">
            {info.data.thumbnail ? (
              <img src={info.data.thumbnail} alt="" className="aspect-video w-44 shrink-0 rounded-md border border-line bg-sunken object-cover" referrerPolicy="no-referrer" />
            ) : (
              <div className="flex aspect-video w-44 shrink-0 items-center justify-center rounded-md border border-line bg-sunken text-faint">
                <Film size={22} />
              </div>
            )}
            <div className="min-w-0 flex-1">
              <h2 className="text-[14px] leading-snug font-semibold break-words selectable">{info.data.title}</h2>
              <p className="mt-1 text-[12.5px] text-muted">
                {[info.data.uploader, info.data.duration ? duration(info.data.duration) : null, info.data.extractor].filter(Boolean).join(' · ')}
              </p>
              {info.data.isPlaylist && (
                <label className="mt-2 flex items-center gap-2 text-[12.5px]">
                  <input type="checkbox" checked={playlist} onChange={(e) => setPlaylist(e.target.checked)} className="accent-[var(--accent)]" />
                  {t('media.playlist', { n: info.data.entries })}
                </label>
              )}
            </div>
          </div>
          <fieldset className="mt-4">
            <legend className="text-[12px] text-muted">{t('media.quality')}</legend>
            <div className="mt-1.5 grid grid-cols-2 gap-1.5 max-[1100px]:grid-cols-1">
              {info.data.choices.map((c) => (
                <label
                  key={c.id}
                  className={clsx(
                    'flex cursor-default items-center gap-2.5 rounded-md border px-3 py-2 text-[13px] transition-colors',
                    choice === c.id ? 'border-ink-2 bg-surface-2' : 'border-line hover:bg-surface-2'
                  )}
                >
                  <input type="radio" name="choice" checked={choice === c.id} onChange={() => setChoice(c.id)} className="accent-[var(--accent)]" />
                  {c.kind === 'audio' ? <Music size={14} className="text-muted" /> : <Film size={14} className="text-muted" />}
                  <span className="flex-1">{c.label}</span>
                  <span className="num text-[12px] text-muted">{c.approxSize ? `≈ ${bytes(c.approxSize, lang)}` : ''}</span>
                </label>
              ))}
            </div>
          </fieldset>
          <div className="mt-3 flex flex-wrap items-center gap-x-5 gap-y-2">
            {info.data.hasSubtitles && !choice.startsWith('audio') && (
              <label className="flex items-center gap-2 text-[12.5px]">
                <input type="checkbox" checked={subs} onChange={(e) => setSubs(e.target.checked)} className="accent-[var(--accent)]" />
                {t('media.subtitles')}
              </label>
            )}
            <span className="flex min-w-0 items-center gap-2 text-[12.5px] text-muted">
              <span className="num truncate">{dir ?? settings?.downloadDir}</span>
              <Button
                size="sm"
                variant="ghost"
                icon={<FolderOpen size={13} />}
                onClick={async () => {
                  const p = await openDialog({ directory: true, defaultPath: dir ?? settings?.downloadDir, title: t('add.chooseFolder') });
                  if (typeof p === 'string') setDir(p);
                }}
              >
                {t('add.change')}
              </Button>
            </span>
            <Button variant="primary" className="ml-auto" onClick={download}>
              {t('media.download')}
            </Button>
          </div>
        </div>
      )}
    </section>
  );
}

const RESOLUTIONS = [0, 2160, 1080, 720, 480];

function Converter() {
  const { t, lang } = useT();
  const pendingPath = useApp((s) => s.pendingConvert);
  const setPendingConvert = useApp((s) => s.setPendingConvert);
  const [probe, setProbe] = useState<FileProbe | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Diagnostic | null>(null);
  const [preset, setPreset] = useState('mp4');
  const [height, setHeight] = useState(0);
  const [start, setStart] = useState('');
  const [end, setEnd] = useState('');

  const load = async (path: string) => {
    setBusy(true);
    setError(null);
    try {
      const p = await api.mediaProbeFile(path);
      setProbe(p);
      setPreset(p.presets.find((x) => x.id === 'mp4')?.id ?? p.presets[0]?.id ?? 'mp3');
      setHeight(0);
      setStart('');
      setEnd('');
    } catch (e) {
      setError(asDiagnostic(e));
      setProbe(null);
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (pendingPath) {
      setPendingConvert(null);
      void load(pendingPath);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingPath]);

  const choose = async () => {
    const p = await openDialog({
      multiple: false,
      filters: [{ name: 'Mídia', extensions: ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'ts', 'wmv', 'flv', 'mp3', 'm4a', 'flac', 'wav', 'ogg', 'opus', 'aac', 'wma'] }]
    });
    if (typeof p === 'string') void load(p);
  };

  const sel = probe?.presets.find((p) => p.id === preset);
  const convert = async () => {
    if (!probe) return;
    try {
      await api.mediaAddConvert(probe.path, preset, {
        height: height || null,
        start: start ? Number(start) : null,
        end: end ? Number(end) : null
      });
      setProbe(null);
    } catch (e) {
      reportError(e);
    }
  };

  return (
    <section className="rounded-lg border border-line bg-surface p-4">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <h2 className="text-[13px] font-medium">{t('media.convertTitle')}</h2>
          <p className="mt-0.5 text-[12px] text-muted">{t('media.convertHelp')}</p>
        </div>
        <Button icon={busy ? <Spinner size={13} /> : <Wand2 size={14} />} onClick={choose} disabled={busy}>
          {t('media.chooseFile')}
        </Button>
      </div>
      {error && <ErrorNote error={error} />}
      {probe && (
        <div className="mt-4 border-t border-line pt-4">
          <div className="text-[13px] font-medium break-words selectable">{probe.name}</div>
          <p className="num mt-0.5 text-[12px] text-muted">
            {[
              probe.info.duration ? duration(probe.info.duration) : null,
              probe.info.video && `${probe.info.video} ${probe.info.width}×${probe.info.height}`,
              probe.info.audio,
              probe.info.size ? bytes(probe.info.size, lang) : null
            ]
              .filter(Boolean)
              .join(' · ')}
          </p>
          <fieldset className="mt-3">
            <legend className="text-[12px] text-muted">{t('media.format')}</legend>
            <div className="mt-1.5 grid grid-cols-3 gap-1.5 max-[1100px]:grid-cols-2">
              {probe.presets.map((p) => (
                <label
                  key={p.id}
                  className={clsx(
                    'flex cursor-default items-center gap-2 rounded-md border px-2.5 py-2 text-[12.5px] transition-colors',
                    preset === p.id ? 'border-ink-2 bg-surface-2' : 'border-line hover:bg-surface-2'
                  )}
                >
                  <input type="radio" name="preset" checked={preset === p.id} onChange={() => setPreset(p.id)} className="accent-[var(--accent)]" />
                  <span className="min-w-0 flex-1 truncate">{p.label}</span>
                </label>
              ))}
            </div>
          </fieldset>
          <div className="mt-3 flex flex-wrap items-end gap-x-5 gap-y-3">
            {sel && !sel.audioOnly && !sel.remux && (
              <label className="text-[12px] text-muted">
                {t('media.resolution')}
                <select value={height} onChange={(e) => setHeight(Number(e.target.value))} className="mt-1 block h-8 rounded-md border border-line-strong bg-surface px-2 text-[13px] text-ink">
                  {RESOLUTIONS.filter((r) => r === 0 || !probe.info.height || r < probe.info.height).map((r) => (
                    <option key={r} value={r}>
                      {r === 0 ? t('media.original') : `${r}p`}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <div className="text-[12px] text-muted">
              {t('media.trim')}
              <div className="mt-1 flex items-center gap-1.5">
                <TextInput mono className="w-20" inputMode="decimal" placeholder="0" value={start} onChange={(e) => setStart(e.target.value.replace(/[^\d.]/g, ''))} aria-label={t('media.from')} />
                <span>{t('media.to')}</span>
                <TextInput mono className="w-20" inputMode="decimal" placeholder={probe.info.duration ? probe.info.duration.toFixed(0) : ''} value={end} onChange={(e) => setEnd(e.target.value.replace(/[^\d.]/g, ''))} aria-label={t('media.to')} />
              </div>
            </div>
            <div className="text-[12px] text-muted">
              {t('media.estimate')}
              <div className="num mt-1 h-8 text-[13px] leading-8 text-ink">{sel?.approxSize ? `≈ ${bytes(sel.approxSize, lang)}` : <span className="font-sans text-muted">{t('media.estimateUnknown')}</span>}</div>
            </div>
            <Button variant="primary" className="ml-auto" onClick={convert}>
              {t('media.convert')}
            </Button>
          </div>
          {preset === 'mp4-hw' && probe.hwEncoder && <p className="mt-2 text-[12px] text-muted">{t('media.hw', { enc: probe.hwEncoder })}</p>}
        </div>
      )}
    </section>
  );
}

function JobRow({ job }: { job: MediaView }) {
  const { t, lang } = useT();
  const err = job.error && job.status === 'failed' ? diagnosticText(job.error, lang) : null;
  const running = job.status === 'running';
  const phase = running && job.phase ? t(`media.phase.${job.phase}` as 'media.phase.video') : t(`mstatus.${job.status}` as 'mstatus.queued');
  const rate = job.speed == null ? '' : job.kind === 'convert' ? t('media.speedx', { x: job.speed.toFixed(1) }) : speed(job.speed, lang);
  return (
    <li className="flex items-center gap-3 border-b border-line px-4 py-3 last:border-b-0">
      {job.thumbnail ? (
        <img src={job.thumbnail} alt="" className="aspect-video w-20 shrink-0 rounded-sm border border-line object-cover" referrerPolicy="no-referrer" />
      ) : (
        <div className="flex aspect-video w-20 shrink-0 items-center justify-center rounded-sm border border-line bg-surface-2 text-muted">{job.kind === 'convert' ? <Wand2 size={15} /> : <Film size={15} />}</div>
      )}
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-medium" title={job.title}>
          {job.kind === 'convert' ? t('media.convertOf', { name: job.title }) : job.title}
        </div>
        <div className="mt-0.5 flex items-center gap-2 text-[12px] text-muted">
          <span className={clsx(job.status === 'failed' && 'text-bad')}>{err ? err.title : phase}</span>
          {job.label && <span>· {job.label}</span>}
          {job.items && job.items > 1 && <span>· {t('media.item', { i: job.item ?? 1, n: job.items })}</span>}
          {running && <span className="num ml-auto">{[rate, job.eta ? duration(job.eta) : ''].filter(Boolean).join(' · ')}</span>}
        </div>
        {(running || job.status === 'paused' || job.status === 'queued') && (
          <div className="mt-1.5 flex items-center gap-2">
            <ProgressBar value={running && job.progress === 0 ? null : job.progress * 100} status={running ? 'downloading' : job.status === 'paused' ? 'paused' : 'queued'} />
            <span className="num w-10 shrink-0 text-right text-[11.5px] text-muted">{percentLabel(job.progress * 100, lang)}</span>
          </div>
        )}
      </div>
      <div className="flex shrink-0 gap-0.5">
        {job.kind === 'download' && running && (
          <IconButton label={t('action.pause')} onClick={() => mediaActions.pause(job)}>
            <Pause size={15} />
          </IconButton>
        )}
        {(job.status === 'paused' || job.status === 'failed') && (
          <IconButton label={job.status === 'failed' ? t('action.retry') : t('action.resume')} onClick={() => mediaActions.resume(job)}>
            {job.status === 'failed' ? <RotateCcw size={15} /> : <Play size={15} />}
          </IconButton>
        )}
        {job.status === 'completed' && (
          <>
            <IconButton label={t('action.open')} onClick={() => mediaActions.open(job)}>
              <SquareArrowOutUpRight size={15} />
            </IconButton>
            <IconButton label={t('action.reveal')} onClick={() => mediaActions.reveal(job)}>
              <FolderSearch size={15} />
            </IconButton>
          </>
        )}
        {(running || job.status === 'queued' || job.status === 'paused') && (
          <IconButton label={t('action.cancel')} tone="danger" onClick={() => mediaActions.cancel(job)}>
            <X size={15} />
          </IconButton>
        )}
        {(job.status === 'completed' || job.status === 'failed' || job.status === 'cancelled') && (
          <IconButton label={t('action.remove')} onClick={() => mediaActions.remove(job)}>
            <Trash2 size={15} />
          </IconButton>
        )}
      </div>
    </li>
  );
}

export function Media() {
  const { t } = useT();
  const media = useApp((s) => s.media);
  const jobs = useMemo(() => Object.values(media).sort((a, b) => b.createdAt - a.createdAt), [media]);
  const [available, setAvailable] = useState(true);
  const checked = useRef(false);
  useEffect(() => {
    if (checked.current) return;
    checked.current = true;
    api.mediaTools().then((x) => setAvailable(x.available)).catch(() => {});
  }, []);

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[980px] px-6 pt-5 pb-10">
        <PageTitle eyebrow={t('nav.groupTools')} title={t('nav.media')} caption={t('media.legal')} />
        {!available && <p className="mt-3 rounded-md bg-bad-soft px-3 py-2 text-[12.5px] text-bad">{t('media.unavailable')}</p>}
        <div className="mt-4 grid gap-4">
          <Analyzer />
          <Converter />
        </div>
        <h2 className="mt-6 text-[11px] font-medium tracking-[0.08em] text-muted uppercase">{t('media.jobs')}</h2>
        <div className="mt-2 rounded-lg border border-line bg-surface">
          {jobs.length === 0 ? <p className="px-4 py-6 text-[12.5px] text-muted">{t('media.empty')}</p> : <ul>{jobs.map((j) => <JobRow key={j.id} job={j} />)}</ul>}
        </div>
      </div>
    </div>
  );
}
