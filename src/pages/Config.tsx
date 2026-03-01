import { useState, useEffect } from "react";
import { api } from "../lib/api";

interface Config {
  device_ip: string;
  device_user: string;
  device_password: string;
  app_url: string;
  api_key: string;
  clock_id: string;
  sync_interval_secs: number;
}

export default function Config() {
  const [deviceIp, setDeviceIp] = useState("");
  const [deviceUser, setDeviceUser] = useState("admin");
  const [devicePassword, setDevicePassword] = useState("");
  const [appUrl, setAppUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [clockId, setClockId] = useState("");
  const [syncInterval, setSyncInterval] = useState(5);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<"success" | "error" | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveMessage, setSaveMessage] = useState<"success" | "error" | null>(null);

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const config = await api.getConfig() as unknown as Config;
      setDeviceIp(config.device_ip || "");
      setDeviceUser(config.device_user || "admin");
      setDevicePassword(config.device_password || "");
      setAppUrl(config.app_url || "");
      setApiKey(config.api_key || "");
      setClockId(config.clock_id || "");
      setSyncInterval(config.sync_interval_secs ? Math.floor(config.sync_interval_secs / 60) : 5);
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setSaveMessage(null);
    try {
      await api.saveConfig({
        device_ip: deviceIp,
        device_user: deviceUser,
        device_password: devicePassword,
        app_url: appUrl,
        api_key: apiKey,
        clock_id: clockId,
        sync_interval_secs: syncInterval * 60,
      });
      setSaveMessage("success");
    } catch (e) {
      console.error("Failed to save config:", e);
      setSaveMessage("error");
    }
    setSaving(false);
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await api.testConnection({
        device_ip: deviceIp,
        device_user: deviceUser,
        device_password: devicePassword,
      });
      setTestResult(result.success ? "success" : "error");
    } catch (e) {
      console.error("Connection test failed:", e);
      setTestResult("error");
    }
    setTesting(false);
  };

  return (
    <div className="px-4 py-6 sm:px-0">
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-lg font-medium mb-6">Configurações</h2>

        <div className="space-y-6">
          <div>
            <h3 className="text-sm font-medium text-gray-700 mb-3">
              Relógio de Ponto (IDClass)
            </h3>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700">
                  IP do Dispositivo
                </label>
                <input
                  type="text"
                  value={deviceIp}
                  onChange={(e) => setDeviceIp(e.target.value)}
                  placeholder="192.168.1.3"
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2 border"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700">
                  Usuário
                </label>
                <input
                  type="text"
                  value={deviceUser}
                  onChange={(e) => setDeviceUser(e.target.value)}
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2 border"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700">
                  Senha
                </label>
                <input
                  type="password"
                  value={devicePassword}
                  onChange={(e) => setDevicePassword(e.target.value)}
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2 border"
                />
              </div>
            </div>
          </div>

          <div className="border-t pt-6">
            <h3 className="text-sm font-medium text-gray-700 mb-3">
              Aplicação ryanne/vendas
            </h3>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700">
                  URL da Aplicação
                </label>
                <input
                  type="text"
                  value={appUrl}
                  onChange={(e) => setAppUrl(e.target.value)}
                  placeholder="https://ryanne.com.br"
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2 border"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700">
                  API Key
                </label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2 border"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700">
                  ID do Relógio
                </label>
                <input
                  type="text"
                  value={clockId}
                  onChange={(e) => setClockId(e.target.value)}
                  placeholder="ID cadastrado em Relógios ponto"
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2 border"
                />
              </div>
            </div>
          </div>

          <div className="border-t pt-6">
            <h3 className="text-sm font-medium text-gray-700 mb-3">
              Sincronização
            </h3>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Intervalo de sync (minutos)
              </label>
              <input
                type="number"
                value={syncInterval}
                onChange={(e) => setSyncInterval(parseInt(e.target.value) || 5)}
                min={1}
                max={60}
                className="mt-1 block w-32 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2 border"
              />
            </div>
          </div>

          <div className="border-t pt-6 flex items-center justify-between">
            <div>
              {testResult === "success" && (
                <span className="text-sm text-green-600">
                  ✓ Conexão bem-sucedida
                </span>
              )}
              {testResult === "error" && (
                <span className="text-sm text-red-600">
                  ✗ Erro na conexão
                </span>
              )}
              {saveMessage === "success" && (
                <span className="text-sm text-green-600">
                  ✓ Configurações salvas
                </span>
              )}
              {saveMessage === "error" && (
                <span className="text-sm text-red-600">
                  ✗ Erro ao salvar
                </span>
              )}
            </div>
            <div className="flex space-x-3">
              <button
                onClick={handleTest}
                disabled={testing || !deviceIp || !devicePassword}
                className="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50"
              >
                {testing ? "Testando..." : "Testar conexão"}
              </button>
              <button
                onClick={handleSave}
                disabled={saving}
                className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50"
              >
                {saving ? "Salvando..." : "Salvar"}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
