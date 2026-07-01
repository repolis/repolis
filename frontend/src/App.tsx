import { useEffect } from "react";

import init, { say_hello_from_rust } from "./wasm/engine";

function App() {
  useEffect(() => {
    async function loadWasm() {
      await init();
      say_hello_from_rust();
    }
    loadWasm();
  }, []);

  return (
    <>
      <div className="flex flex-col gap-1 px-4">
        <h1 className="text-3xl font-semibold text-blue-500">Repolis</h1>
        <input className="rounded-md border-2" />
      </div>
    </>
  );
}

export default App;
