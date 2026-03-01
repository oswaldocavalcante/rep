import { useState, useEffect } from "react";
import { api } from "../lib/api";

interface LogEntry {
  id: number;
  timestamp: string;
  status: string;
  records_sent: number;
  message: string;
}

export default function Logs() {
  const [filter, setFilter] = useState<"all" | "success" | "error" | "info">("all");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [live, setLive] = useState(true);

  useEffect(() => {
    loadLogs();
  }, []);

  useEffect(() => {
    if (!live) return;
    const timer = setInterval(() => {
      loadLogs(false);
    }, 2000);
    return () => clearInterval(timer);
  }, [live]);

  const loadLogs = async (showLoading = true) => {
    if (showLoading) setLoading(true);
    try {
      const result = await api.getLogs();
      setLogs(result);
    } catch (e) {
      console.error("Failed to load logs:", e);
    }
    if (showLoading) setLoading(false);
  };

  const filteredLogs = logs.filter((log) => {
    if (filter === "all") return true;
    return log.status === filter;
  });

  return (
    <div className="px-4 py-6 sm:px-0">
      <div className="bg-white shadow rounded-lg p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-medium">Logs de Sincronização</h2>
          <div className="flex items-center gap-2">
            <label className="text-xs text-gray-600 inline-flex items-center gap-1">
              <input type="checkbox" checked={live} onChange={(e) => setLive(e.target.checked)} />
              Tempo real
            </label>
            <button
              onClick={() => loadLogs()}
              className="px-3 py-1 text-sm bg-gray-100 rounded-md hover:bg-gray-200"
            >
              Atualizar
            </button>
          </div>
        </div>

        <div className="flex space-x-2 mb-4">
            <button
              onClick={() => setFilter("all")}
              className={`px-3 py-1 text-sm rounded-md ${
                filter === "all"
                  ? "bg-gray-800 text-white"
                  : "bg-gray-100 text-gray-700"
              }`}
            >
              Todos
            </button>
            <button
              onClick={() => setFilter("success")}
              className={`px-3 py-1 text-sm rounded-md ${
                filter === "success"
                  ? "bg-green-600 text-white"
                  : "bg-gray-100 text-gray-700"
              }`}
            >
              Sucesso
            </button>
            <button
              onClick={() => setFilter("error")}
              className={`px-3 py-1 text-sm rounded-md ${
                filter === "error"
                  ? "bg-red-600 text-white"
                  : "bg-gray-100 text-gray-700"
              }`}
            >
              Erro
            </button>
            <button
              onClick={() => setFilter("info")}
              className={`px-3 py-1 text-sm rounded-md ${
                filter === "info"
                  ? "bg-blue-600 text-white"
                  : "bg-gray-100 text-gray-700"
              }`}
            >
              Info
            </button>
        </div>

        {loading ? (
          <p className="text-center py-8 text-gray-500">Carregando...</p>
        ) : filteredLogs.length === 0 ? (
          <p className="text-center py-8 text-gray-500">
            Nenhum log encontrado
          </p>
        ) : (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Data/Hora
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Status
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Registros
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Mensagem
                  </th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {filteredLogs.map((log) => (
                  <tr key={log.id}>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {log.timestamp}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <span
                        className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
                          log.status === "success"
                            ? "bg-green-100 text-green-800"
                            : log.status === "info"
                            ? "bg-blue-100 text-blue-800"
                            : "bg-red-100 text-red-800"
                        }`}
                      >
                        {log.status === "success" ? "Sucesso" : log.status === "info" ? "Info" : "Erro"}
                      </span>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {log.records_sent}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                      {log.message}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
