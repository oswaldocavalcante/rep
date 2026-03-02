import { useState, useEffect } from "react";
import { api } from "../lib/api";

interface SyncResult {
  success: boolean;
  records_sent: number;
  message: string;
}

interface SyncStatus {
  last_synced_at: string | null;
  last_nsr: number;
  last_records_sent: number;
  last_message: string;
  sync_interval_secs: number;
  next_sync_at: string | null;
}

interface LogEntry {
  id: number;
  timestamp: string;
  status: string;
  records_sent: number;
  message: string;
}

interface VersionInfo {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  release_url: string;
  error?: string;
}

export default function Status() {
  const [lastSync, setLastSync] = useState<string>("--");
  const [recordsSent, setRecordsSent] = useState<number>(0);
  const [nextSync, setNextSync] = useState<string>("--");
  const [status, setStatus] = useState<"ok" | "syncing" | "error">("ok");
  const [errorMessage, setErrorMessage] = useState<string>("");
  const [showCollected, setShowCollected] = useState(false);
  const [collectedText, setCollectedText] = useState("Nenhuma coleta registrada ainda.");
  const [versionInfo, setVersionInfo] = useState<VersionInfo | null>(null);
  const [updating, setUpdating] = useState(false);
  const [updateOutput, setUpdateOutput] = useState<string | null>(null);

  useEffect(() => {
    loadState();
    loadVersion();
  }, []);

  const loadVersion = async () => {
    try {
      const info = await api.getVersion();
      setVersionInfo(info);
    } catch {
      // silencioso — versão é informativa
    }
  };

  const handleUpdate = async () => {
    setUpdating(true);
    setUpdateOutput(null);
    try {
      const result = await api.triggerUpdate();
      setUpdateOutput(result.output);
      if (result.success) {
        // Recarrega info de versão após alguns segundos (serviço reinicia)
        setTimeout(() => loadVersion(), 4000);
      }
    } catch (e: unknown) {
      setUpdateOutput(e instanceof Error ? e.message : String(e));
    } finally {
      setUpdating(false);
    }
  };

  useEffect(() => {
    if (!showCollected) return;
    loadCollectedPreview();
    const timer = setInterval(() => {
      loadCollectedPreview();
    }, 2000);
    return () => clearInterval(timer);
  }, [showCollected]);

  const loadState = async () => {
    try {
      const statusData = await api.getStatus() as unknown as SyncStatus;
      setLastSync(
        statusData.last_synced_at
          ? new Date(statusData.last_synced_at).toLocaleString("pt-BR")
          : "--"
      );
      setNextSync(
        statusData.next_sync_at
          ? new Date(statusData.next_sync_at).toLocaleTimeString("pt-BR")
          : "--"
      );
      setRecordsSent(statusData.last_records_sent || 0);
      if (statusData.last_message) {
        setErrorMessage(statusData.last_message);
      }
    } catch (e) {
      console.error("Failed to load state:", e);
    }
  };

  const handleSync = async () => {
    setStatus("syncing");
    setErrorMessage("");
    try {
      const result: SyncResult = await api.syncNow();
      await loadState();
      await loadCollectedPreview();
      setStatus(result.success ? "ok" : "error");
      setErrorMessage(result.message);
    } catch (e: unknown) {
      setStatus("error");
      setErrorMessage(e instanceof Error ? e.message : String(e));
    }
  };

  const loadCollectedPreview = async () => {
    try {
      const logs: LogEntry[] = await api.getLogs();
      const preview = logs.find((log) => log.message?.startsWith("COLETA_PREVIEW"));
      if (preview) {
        setCollectedText(`[${preview.timestamp}]\n${preview.message}`);
      }
    } catch (e) {
      console.error("Failed to load collected preview:", e);
    }
  };

  const handleResetHistory = async () => {
    const confirmed = window.confirm(
      "Isso vai resetar o cursor de sincronização e permitir reprocessar o histórico do relógio. Deseja continuar?"
    );
    if (!confirmed) return;

    setStatus("syncing");
    setErrorMessage("");
    try {
      const result: SyncResult = await api.reprocessHistory();
      await loadState();
      await loadCollectedPreview();
      setStatus("ok");
      setErrorMessage(`Reprocessamento executado. ${result.message}`);
    } catch (e: unknown) {
      setStatus("error");
      setErrorMessage(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="px-4 py-6 sm:px-0">
      {/* Banner de atualização disponível */}
      {versionInfo?.update_available && (
        <div className="mb-4 bg-blue-50 border border-blue-200 rounded-lg px-4 py-3 space-y-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-blue-600 text-lg">↑</span>
              <span className="text-sm text-blue-800">
                Nova versão disponível:{" "}
                <strong>v{versionInfo.latest_version}</strong>{" "}
                (atual: v{versionInfo.current_version})
              </span>
            </div>
            <div className="flex items-center gap-3">
              <a
                href={versionInfo.release_url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-blue-700 font-medium hover:underline"
              >
                Ver release →
              </a>
              <button
                onClick={handleUpdate}
                disabled={updating}
                className="px-3 py-1.5 text-sm bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50"
              >
                {updating ? "Atualizando..." : "Atualizar agora"}
              </button>
            </div>
          </div>
          {updateOutput && (
            <pre className="text-xs font-mono bg-blue-100 rounded p-2 whitespace-pre-wrap text-blue-900 max-h-40 overflow-y-auto">
              {updateOutput}
            </pre>
          )}
        </div>
      )}
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-lg font-medium mb-4">Status da Sincronização</h2>
        
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <div className="bg-gray-50 p-4 rounded-lg">
            <p className="text-sm text-gray-500">Última sincronização</p>
            <p className="text-lg font-medium">{lastSync}</p>
          </div>
          <div className="bg-gray-50 p-4 rounded-lg">
            <p className="text-sm text-gray-500">Registros enviados</p>
            <p className="text-lg font-medium">{recordsSent}</p>
          </div>
          <div className="bg-gray-50 p-4 rounded-lg">
            <p className="text-sm text-gray-500">Próxima sincronização</p>
            <p className="text-lg font-medium">{nextSync}</p>
          </div>
        </div>

        <div className="flex items-center justify-between">
          <div className="flex items-center">
            <span
              className={`inline-flex items-center px-3 py-1 rounded-full text-sm font-medium ${
                status === "ok"
                  ? "bg-green-100 text-green-800"
                  : status === "syncing"
                  ? "bg-yellow-100 text-yellow-800"
                  : "bg-red-100 text-red-800"
              }`}
            >
              {status === "ok" && "✓ OK"}
              {status === "syncing" && "⟳ Sincronizando"}
              {status === "error" && "✗ Erro"}
            </span>
            {errorMessage && (
              <span className="ml-3 text-sm text-red-600">{errorMessage}</span>
            )}
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={handleResetHistory}
              disabled={status === "syncing"}
              className="px-4 py-2 bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200 disabled:opacity-50"
            >
              Reprocessar histórico
            </button>
            <button
              onClick={handleSync}
              disabled={status === "syncing"}
              className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50"
            >
              {status === "syncing" ? "Sincronizando..." : "Sincronizar agora"}
            </button>
          </div>
        </div>

        <div className="mt-6">
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-sm font-medium text-gray-700">Dados coletados na sincronização</h3>
            <button
              onClick={() => setShowCollected((prev) => !prev)}
              className="px-3 py-1 text-sm bg-gray-100 rounded-md hover:bg-gray-200"
            >
              {showCollected ? "Ocultar" : "Mostrar"}
            </button>
          </div>

          {showCollected && (
            <textarea
              readOnly
              value={collectedText}
              className="w-full h-56 rounded-md border border-gray-300 bg-gray-50 p-3 text-xs font-mono text-gray-800"
            />
          )}
        </div>

        {/* Versão atual */}
        <div className="mt-4 pt-3 border-t border-gray-100 flex items-center justify-between">
          <span className="text-xs text-gray-400">
            rep-server{versionInfo ? ` v${versionInfo.current_version}` : ""}
          </span>
          {versionInfo && !versionInfo.update_available && (
            <span className="text-xs text-green-600">✓ Atualizado</span>
          )}
        </div>
      </div>
    </div>
  );
}
