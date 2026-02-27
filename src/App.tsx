import { useState } from "react";
import Config from "./pages/Config";
import Status from "./pages/Status";
import Logs from "./pages/Logs";

type Page = "config" | "status" | "logs";

function App() {
  const [currentPage, setCurrentPage] = useState<Page>("status");

  return (
    <div className="min-h-screen bg-gray-100">
      <header className="bg-white shadow">
        <div className="max-w-7xl mx-auto py-4 px-4 sm:px-6 lg:px-8">
          <h1 className="text-2xl font-bold text-gray-900">
            Ryanne Ponto Agent
          </h1>
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
