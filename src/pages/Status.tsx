import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SyncResult {
  success: boolean;
  records_sent: number;
  message: string;
}

interface Config {
  device_ip: string;
  device_user: string;
  device_password: string;
  app_url: string;
  api_key: string;
  sync_interval_secs: number;
}

export default function Status() {
  const [lastSync, setLastSync] = useState<string>("--");
  const [recordsSent, setRecordsSent] = useState<number>(0);
  const [nextSync, setNextSync] = useState<string>("--");
  const [status, setStatus] = useState<"ok" | "syncing" | "error">("ok");
  const [errorMessage, setErrorMessage] = useState<string>("");

  useEffect(() => {
    loadState();
  }, []);

  const loadState = async () => {
    try {
      const config = await invoke<Config>("load_config");
      if (config.device_ip) {
        setLastSync(new Date().toLocaleString("pt-BR"));
        const interval = (config.sync_interval_secs || 300) / 60;
        const next = new Date(Date.now() + interval * 60 * 1000);
        setNextSync(next.toLocaleTimeString("pt-BR"));
      }
    } catch (e) {
      console.error("Failed to load state:", e);
    }
  };

  const handleSync = async () => {
    setStatus("syncing");
    setErrorMessage("");
    try {
      const result: SyncResult = await invoke("sync_now");
      setRecordsSent((prev) => prev + result.records_sent);
      setLastSync(new Date().toLocaleString("pt-BR"));
      
      const config = await invoke<Config>("load_config");
      const interval = (config.sync_interval_secs || 300) / 60;
      const next = new Date(Date.now() + interval * 60 * 1000);
      setNextSync(next.toLocaleTimeString("pt-BR"));
      
      setStatus(result.success ? "ok" : "error");
      setErrorMessage(result.message);
    } catch (e: any) {
      setStatus("error");
      setErrorMessage(e.toString());
    }
  };

  return (
    <div className="px-4 py-6 sm:px-0">
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

          <button
            onClick={handleSync}
            disabled={status === "syncing"}
            className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50"
          >
            {status === "syncing" ? "Sincronizando..." : "Sincronizar agora"}
          </button>
        </div>
      </div>
    </div>
  );
}
