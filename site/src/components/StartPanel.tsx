import React, { useEffect, useMemo, useState } from 'react';

function getInitialUrl(): string {
  if (typeof window === 'undefined') return '';
  try {
    const saved = localStorage.getItem('triad_api_url');
    if (saved) return saved;
  } catch {}
  // Allow setting from build-time env if provided
  // In Astro, PUBLIC_* are exposed to client
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const env = (import.meta as any).env || {};
  return env.PUBLIC_TRIAD_API || '';
}

type FetchState<T> = {
  loading: boolean;
  data?: T;
  error?: string;
};

export default function StartPanel() {
  const [apiUrl, setApiUrl] = useState<string>('');
  const [savedUrl, setSavedUrl] = useState<string>('');
  const [health, setHealth] = useState<FetchState<any>>({ loading: false });
  const [peers, setPeers] = useState<FetchState<any[]>>({ loading: false });
  const [txPayload, setTxPayload] = useState<string>('{"kind":"test_wave","payload":"hello"}');
  const [txResult, setTxResult] = useState<FetchState<any>>({ loading: false });

  useEffect(() => {
    setApiUrl(getInitialUrl());
    setSavedUrl(getInitialUrl());
  }, []);

  const baseOk = useMemo(() => {
    try {
      return !!apiUrl && new URL(apiUrl).protocol.startsWith('http');
    } catch {
      return false;
    }
  }, [apiUrl]);

  function persistUrl() {
    try {
      localStorage.setItem('triad_api_url', apiUrl);
      setSavedUrl(apiUrl);
    } catch {}
  }

  async function doHealth() {
    if (!baseOk) return;
    setHealth({ loading: true });
    try {
      const res = await fetch(apiUrl.replace(/\/$/, '') + '/health', { cache: 'no-cache' });
      const txt = await res.text();
      let data: any;
      try { data = JSON.parse(txt); } catch { data = { raw: txt }; }
      setHealth({ loading: false, data });
    } catch (e: any) {
      setHealth({ loading: false, error: e?.message || 'Network error' });
    }
  }

  async function doPeers() {
    if (!baseOk) return;
    setPeers({ loading: true });
    try {
      const res = await fetch(apiUrl.replace(/\/$/, '') + '/peers', { cache: 'no-cache' });
      const json = await res.json();
      const arr = Array.isArray(json) ? json : (json?.peers || []);
      setPeers({ loading: false, data: arr });
    } catch (e: any) {
      setPeers({ loading: false, error: e?.message || 'Network error' });
    }
  }

  async function doSubmit() {
    if (!baseOk) return;
    setTxResult({ loading: true });
    try {
      const body = txPayload ? JSON.parse(txPayload) : {};
      const res = await fetch(apiUrl.replace(/\/$/, '') + '/submit', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const txt = await res.text();
      let data: any;
      try { data = JSON.parse(txt); } catch { data = { raw: txt }; }
      setTxResult({ loading: false, data });
    } catch (e: any) {
      setTxResult({ loading: false, error: e?.message || 'Network/JSON error' });
    }
  }

  return (
    <div className="rounded-xl border border-white/10 bg-white/5 p-5 space-y-5">
      <div>
        <h2 className="text-xl font-semibold text-white">Взаимодействие с сетью</h2>
        <p className="mt-1 text-mist text-sm">Укажите URL API работающего узла TRIAD (например, http://localhost:8080). Данные сохранятся в этом браузере.</p>
        <div className="mt-3 grid gap-2 sm:grid-cols-[1fr_auto] items-center">
          <input
            value={apiUrl}
            onChange={(e) => setApiUrl(e.target.value)}
            placeholder="http://localhost:8080"
            className="w-full rounded-lg bg-black/30 border border-white/10 px-3 py-2 text-white placeholder-white/30 outline-none focus:ring-2 focus:ring-white/20"
          />
          <button onClick={persistUrl} className="rounded-lg bg-white/10 hover:bg-white/15 text-white px-4 py-2 transition disabled:opacity-50" disabled={!baseOk}>
            Сохранить
          </button>
        </div>
        {savedUrl && (
          <p className="mt-2 text-xs text-mist">Сохранено: {savedUrl}</p>
        )}
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="rounded-lg border border-white/10 bg-black/20 p-4">
          <div className="flex items-center justify-between">
            <h3 className="text-white font-medium">Статус /health</h3>
            <button onClick={doHealth} disabled={!baseOk || health.loading} className="rounded bg-white/10 hover:bg-white/15 px-3 py-1 text-sm disabled:opacity-50">Проверить</button>
          </div>
          <pre className="mt-2 text-xs overflow-auto max-h-52 whitespace-pre-wrap text-mist">{health.loading ? 'Загрузка…' : health.error ? `Ошибка: ${health.error}` : JSON.stringify(health.data, null, 2) || '—'}</pre>
        </div>
        <div className="rounded-lg border border-white/10 bg-black/20 p-4">
          <div className="flex items-center justify-between">
            <h3 className="text-white font-medium">Соседи /peers</h3>
            <button onClick={doPeers} disabled={!baseOk || peers.loading} className="rounded bg-white/10 hover:bg-white/15 px-3 py-1 text-sm disabled:opacity-50">Получить</button>
          </div>
          <pre className="mt-2 text-xs overflow-auto max-h-52 whitespace-pre-wrap text-mist">{peers.loading ? 'Загрузка…' : peers.error ? `Ошибка: ${peers.error}` : JSON.stringify(peers.data, null, 2) || '—'}</pre>
        </div>
      </div>

      <div className="rounded-lg border border-white/10 bg-black/20 p-4 space-y-2">
        <div className="flex items-center justify-between">
          <h3 className="text-white font-medium">Тестовая отправка /submit</h3>
          <button onClick={doSubmit} disabled={!baseOk || txResult.loading} className="rounded bg-white/10 hover:bg-white/15 px-3 py-1 text-sm disabled:opacity-50">Отправить</button>
        </div>
        <textarea
          value={txPayload}
          onChange={(e) => setTxPayload(e.target.value)}
          rows={4}
          className="w-full rounded bg-black/30 border border-white/10 px-3 py-2 text-white text-xs"
        />
        <pre className="text-xs overflow-auto max-h-52 whitespace-pre-wrap text-mist">{txResult.loading ? 'Отправка…' : txResult.error ? `Ошибка: ${txResult.error}` : JSON.stringify(txResult.data, null, 2) || '—'}</pre>
      </div>

      {!baseOk && (
        <p className="text-xs text-amber-300/80">Укажите корректный адрес (http/https). Пример: http://localhost:8080</p>
      )}
    </div>
  );
}
