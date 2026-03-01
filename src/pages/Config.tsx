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

// ── Componente de campo de formulário ─────────────────────────────────────────
function Field({
  label, value, onChange, type = "text", placeholder, readOnly,
}: {
  label: string; value: string; onChange?: (v: string) => void;
  type?: string; placeholder?: string; readOnly?: boolean;
}) {
  return (
    <div>
      <label className="block text-sm font-medium text-gray-700 mb-1">{label}</label>
      <input
        type={type}
        value={value}
        readOnly={readOnly}
        onChange={e => onChange?.(e.target.value)}
        placeholder={placeholder}
        className={`block w-full rounded-md border px-3 py-2 text-sm shadow-sm
          ${readOnly
            ? "bg-gray-50 border-gray-200 text-gray-500 cursor-default"
            : "border-gray-300 focus:border-blue-500 focus:ring-blue-500"}`}
      />
    </div>
  );
}

function StatusBadge({ type, message }: { type: "success" | "error" | "info"; message: string }) {
  const styles = {
    success: "text-green-600",
    error: "text-red-600",
    info: "text-blue-600",
  };
  const icons = { success: "✓", error: "✗", info: "⟳" };
  return <span className={`text-sm ${styles[type]}`}>{icons[type]} {message}</span>;
}

// ── Página ────────────────────────────────────────────────────────────────────
export default function Config() {
  // Integração
  const [apiKey, setApiKey]   = useState("");
  const [clockId, setClockId] = useState("");

  // Dispositivo (preenchido pelo provisionamento, editável manualmente)
  const [deviceIp, setDeviceIp]           = useState("");
  const [deviceUser, setDeviceUser]       = useState("");
  const [devicePassword, setDevicePassword] = useState("");
  const [showDeviceForm, setShowDeviceForm] = useState(false);

  // Sincronização
  const [syncInterval, setSyncInterval] = useState(5);

  // Segurança
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword]         = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  // Estados de UI
  const [provisioning, setProvisioning]   = useState(false);
  const [provisionMsg, setProvisionMsg]   = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [testing, setTesting]             = useState(false);
  const [testResult, setTestResult]       = useState<"success" | "error" | null>(null);
  const [saving, setSaving]               = useState(false);
  const [saveMsg, setSaveMsg]             = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [changingPwd, setChangingPwd]     = useState(false);
  const [pwdMsg, setPwdMsg]               = useState<{ type: "success" | "error"; text: string } | null>(null);

  useEffect(() => { loadConfig(); }, []);

  const loadConfig = async () => {
    try {
      const c = await api.getConfig() as unknown as Config;
      setApiKey(c.api_key || "");
      setClockId(c.clock_id || "");
      setDeviceIp(c.device_ip || "");
      setDeviceUser(c.device_user || "");
      setDevicePassword(c.device_password || "");
      setSyncInterval(c.sync_interval_secs ? Math.floor(c.sync_interval_secs / 60) : 5);
    } catch (e) { console.error("Failed to load config:", e); }
  };

  // ── Provisionamento ─────────────────────────────────────────────────────────
  const handleProvision = async () => {
    if (!apiKey || !clockId) {
      setProvisionMsg({ type: "error", text: "Preencha a chave de API e o ID do relógio" });
      return;
    }
    setProvisioning(true);
    setProvisionMsg(null);
    try {
      const result = await api.provision({ api_key: apiKey, clock_id: clockId });
      if (result.success) {
        setProvisionMsg({ type: "success", text: `Provisionado com sucesso. IP: ${result.ipAddress}` });
        await loadConfig();
      } else {
        setProvisionMsg({ type: "error", text: result.error || "Erro no provisionamento" });
      }
    } catch (e: unknown) {
      setProvisionMsg({ type: "error", text: e instanceof Error ? e.message : "Erro no provisionamento" });
    }
    setProvisioning(false);
  };

  // ── Salva config de dispositivo/sync manualmente ────────────────────────────
  const handleSave = async () => {
    setSaving(true);
    setSaveMsg(null);
    try {
      await api.saveConfig({
        api_key: apiKey,
        clock_id: clockId,
        device_ip: deviceIp,
        device_user: deviceUser,
        device_password: devicePassword,
        sync_interval_secs: syncInterval * 60,
      });
      setSaveMsg({ type: "success", text: "Configurações salvas" });
    } catch (e: unknown) {
      setSaveMsg({ type: "error", text: e instanceof Error ? e.message : "Erro ao salvar" });
    }
    setSaving(false);
  };

  // ── Teste de conexão ────────────────────────────────────────────────────────
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
    } catch { setTestResult("error"); }
    setTesting(false);
  };

  // ── Troca de senha ──────────────────────────────────────────────────────────
  const handleChangePassword = async () => {
    if (!newPassword || newPassword !== confirmPassword) {
      setPwdMsg({ type: "error", text: "As senhas não coincidem" });
      return;
    }
    setChangingPwd(true);
    setPwdMsg(null);
    try {
      await api.changePassword(currentPassword, newPassword);
      setPwdMsg({ type: "success", text: "Senha alterada com sucesso" });
      setCurrentPassword(""); setNewPassword(""); setConfirmPassword("");
    } catch (e: unknown) {
      setPwdMsg({ type: "error", text: e instanceof Error ? e.message : "Erro ao alterar senha" });
    }
    setChangingPwd(false);
  };

  return (
    <div className="px-4 py-6 sm:px-0 space-y-6">

      {/* ── Integração com o sistema ────────────────────────────────────────── */}
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-base font-semibold text-gray-900 mb-1">Integração com o sistema</h2>
        <p className="text-sm text-gray-500 mb-4">
          Informe as credenciais do sistema Ryanne e clique em <strong>Provisionar</strong> para que
          o agente busque automaticamente o IP e as credenciais do relógio.
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
          <Field label="Chave de API" value={apiKey} onChange={setApiKey} type="password" placeholder="••••••••" />
          <Field label="ID do relógio" value={clockId} onChange={setClockId} placeholder="UUID cadastrado no sistema" />
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={handleProvision}
            disabled={provisioning}
            className="px-4 py-2 bg-blue-600 text-white text-sm rounded-md hover:bg-blue-700 disabled:opacity-50"
          >
            {provisioning ? "Provisionando..." : "Provisionar"}
          </button>
          {provisionMsg && <StatusBadge type={provisionMsg.type} message={provisionMsg.text} />}
        </div>
      </div>

      {/* ── Dispositivo ─────────────────────────────────────────────────────── */}
      <div className="bg-white shadow rounded-lg p-6">
        <div className="flex items-center justify-between mb-1">
          <h2 className="text-base font-semibold text-gray-900">Dispositivo IDClass</h2>
          <button
            onClick={() => setShowDeviceForm(v => !v)}
            className="text-xs text-blue-600 hover:underline"
          >
            {showDeviceForm ? "Ocultar configuração manual" : "Configurar manualmente"}
          </button>
        </div>
        <p className="text-sm text-gray-500 mb-4">
          Credenciais obtidas automaticamente pelo provisionamento.
        </p>

        {/* Read-only resumo */}
        {!showDeviceForm && (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
            <Field label="IP do dispositivo" value={deviceIp} readOnly />
            <Field label="Usuário" value={deviceUser} readOnly />
            <Field label="Senha" value={devicePassword} type="password" readOnly />
          </div>
        )}

        {/* Formulário manual (expansível) */}
        {showDeviceForm && (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
            <Field label="IP do dispositivo" value={deviceIp} onChange={setDeviceIp} placeholder="192.168.1.10" />
            <Field label="Usuário" value={deviceUser} onChange={setDeviceUser} />
            <Field label="Senha" value={devicePassword} onChange={setDevicePassword} type="password" />
          </div>
        )}

        <div className="flex items-center gap-3">
          <button
            onClick={handleTest}
            disabled={testing || !deviceIp}
            className="px-4 py-2 border border-gray-300 text-sm rounded-md text-gray-700 hover:bg-gray-50 disabled:opacity-50"
          >
            {testing ? "Testando..." : "Testar conexão"}
          </button>
          {testResult === "success" && <StatusBadge type="success" message="Conexão bem-sucedida" />}
          {testResult === "error"   && <StatusBadge type="error"   message="Falha na conexão" />}
        </div>
      </div>

      {/* ── Sincronização ───────────────────────────────────────────────────── */}
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-base font-semibold text-gray-900 mb-4">Sincronização</h2>
        <div className="flex items-end gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Intervalo (minutos)
            </label>
            <input
              type="number"
              value={syncInterval}
              onChange={e => setSyncInterval(parseInt(e.target.value) || 5)}
              min={1} max={60}
              className="block w-28 rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:ring-blue-500"
            />
          </div>
          <div className="flex items-center gap-3 pb-0.5">
            <button
              onClick={handleSave}
              disabled={saving}
              className="px-4 py-2 bg-blue-600 text-white text-sm rounded-md hover:bg-blue-700 disabled:opacity-50"
            >
              {saving ? "Salvando..." : "Salvar configurações"}
            </button>
            {saveMsg && <StatusBadge type={saveMsg.type} message={saveMsg.text} />}
          </div>
        </div>
      </div>

      {/* ── Segurança ───────────────────────────────────────────────────────── */}
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-base font-semibold text-gray-900 mb-4">Segurança</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
          <Field label="Senha atual"    value={currentPassword} onChange={setCurrentPassword} type="password" placeholder="••••••••" />
          <Field label="Nova senha"     value={newPassword}     onChange={setNewPassword}     type="password" placeholder="••••••••" />
          <Field label="Confirmar nova senha" value={confirmPassword} onChange={setConfirmPassword} type="password" placeholder="••••••••" />
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={handleChangePassword}
            disabled={changingPwd || !currentPassword || !newPassword}
            className="px-4 py-2 bg-gray-800 text-white text-sm rounded-md hover:bg-gray-900 disabled:opacity-50"
          >
            {changingPwd ? "Alterando..." : "Alterar senha"}
          </button>
          {pwdMsg && <StatusBadge type={pwdMsg.type} message={pwdMsg.text} />}
        </div>
      </div>

    </div>
  );
}
