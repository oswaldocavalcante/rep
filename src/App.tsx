import { useState, useEffect } from "react";
import Config from "./pages/Config";
import Status from "./pages/Status";
import Logs from "./pages/Logs";
import Login from "./pages/Login";
import { api, clearToken, getToken } from "./lib/api";

type Page = "config" | "status" | "logs";

function App() {
  const [currentPage, setCurrentPage] = useState<Page>("status");
  // null = verificando, false = não autenticado, true = autenticado
  const [authenticated, setAuthenticated] = useState<boolean | null>(null);

  useEffect(() => {
    // Verifica sessão existente
    if (!getToken()) {
      setAuthenticated(false);
      return;
    }
    api.me()
      .then(() => setAuthenticated(true))
      .catch(() => setAuthenticated(false));

    // Ouve evento de 401 disparado por apiFetch
    const handleUnauthorized = () => setAuthenticated(false);
    window.addEventListener("rep:unauthorized", handleUnauthorized);
    return () => window.removeEventListener("rep:unauthorized", handleUnauthorized);
  }, []);

  const handleLogout = async () => {
    try {
      await api.logout();
    } catch {
      // ignora erro no logout
    }
    clearToken();
    setAuthenticated(false);
  };

  // Aguardando verificação
  if (authenticated === null) {
    return (
      <div className="min-h-screen bg-gray-100 flex items-center justify-center">
        <p className="text-gray-500">Carregando...</p>
      </div>
    );
  }

  // Não autenticado
  if (!authenticated) {
    return <Login onLogin={() => setAuthenticated(true)} />;
  }

  return (
    <div className="min-h-screen bg-gray-100">
      <header className="bg-white shadow">
        <div className="max-w-7xl mx-auto py-4 px-4 sm:px-6 lg:px-8 flex items-center justify-between">
          <h1 className="text-2xl font-bold text-gray-900">
            Ryanne Ponto Agent
          </h1>
          <button
            onClick={handleLogout}
            className="text-sm text-gray-500 hover:text-gray-700"
          >
            Sair
          </button>
        </div>
      </header>

      <nav className="bg-white shadow-sm">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex space-x-8">
            <button
              onClick={() => setCurrentPage("status")}
              className={`py-4 px-1 border-b-2 font-medium text-sm ${
                currentPage === "status"
                  ? "border-blue-500 text-blue-600"
                  : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300"
              }`}
            >
              Status
            </button>
            <button
              onClick={() => setCurrentPage("config")}
              className={`py-4 px-1 border-b-2 font-medium text-sm ${
                currentPage === "config"
                  ? "border-blue-500 text-blue-600"
                  : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300"
              }`}
            >
              Configurações
            </button>
            <button
              onClick={() => setCurrentPage("logs")}
              className={`py-4 px-1 border-b-2 font-medium text-sm ${
                currentPage === "logs"
                  ? "border-blue-500 text-blue-600"
                  : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300"
              }`}
            >
              Logs
            </button>
          </div>
        </div>
      </nav>

      <main className="max-w-7xl mx-auto py-6 sm:px-6 lg:px-8">
        {currentPage === "status" && <Status />}
        {currentPage === "config" && <Config />}
        {currentPage === "logs" && <Logs />}
      </main>
    </div>
  );
}

export default App;

